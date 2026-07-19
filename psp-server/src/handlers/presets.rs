use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

#[derive(Debug, serde::Deserialize)]
pub struct UpdatePresetData {
    pub id: String,
    pub name: String,
}

/// The `data/json/presets.json` seed array. Entries carry no `id`; `add`
/// generates one per entry.
fn presets_seed(ctx: &HandlerCtx<'_>) -> serde_json::Value {
    ctx.app
        .game_data
        .get("presets")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]))
}

/// Seeds the table from the JSON fixture only when it is empty, then answers
/// with every preset as an id-keyed object (not an array).
pub async fn handle_get_presets(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let seed = presets_seed(ctx);
    psp_db::presets::populate_from_json(&ctx.app.db, &seed).await?;
    let presets = psp_db::presets::get_all(&ctx.app.db).await?;
    ctx.emitter.emit(MessageType::GetPresets, &presets);
    Ok(())
}

pub async fn handle_add_preset(
    data: serde_json::Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let preset_id = psp_db::presets::add(&ctx.app.db, data).await?;
    ctx.emitter.emit(
        MessageType::AddPreset,
        &serde_json::json!({"message": "Preset added successfully", "id": preset_id}),
    );
    Ok(())
}

pub async fn handle_update_preset(
    data: UpdatePresetData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let success = psp_db::presets::update_name(&ctx.app.db, &data.id, &data.name).await?;
    if success {
        ctx.emitter.emit(
            MessageType::UpdatePreset,
            &format!("{} updated successfully", data.name),
        );
    } else {
        // Plain-string error data, not the dispatcher's `{message, trace}`.
        ctx.emitter.emit(
            MessageType::Error,
            &format!("Failed to update preset {}", data.id),
        );
    }
    Ok(())
}

pub async fn handle_delete_presets(
    data: Vec<String>,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let mut success = true;
    for preset_id in &data {
        if !psp_db::presets::delete(&ctx.app.db, preset_id).await? {
            success = false;
        }
    }
    if success {
        ctx.emitter
            .emit(MessageType::DeletePreset, &"Presets deleted successfully");
    } else {
        ctx.emitter
            .emit(MessageType::Error, &"Failed to delete one or more presets");
    }
    Ok(())
}

pub async fn handle_nuke_presets(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    psp_db::presets::nuke(&ctx.app.db).await?;
    ctx.emitter
        .emit(MessageType::NukePresets, &"Presets nuked successfully");
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct ExportPresetData {
    pub preset_id: String,
    pub preset_type: String,
    pub preset_name: String,
}

/// Export and import both need a native file dialog, so outside desktop mode
/// they answer `error` with the plain string "File dialog not available".
pub async fn handle_export_preset(
    data: ExportPresetData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let presets = psp_db::presets::get_all(&ctx.app.db).await?;
    let Some(preset) = presets.get(&data.preset_id) else {
        ctx.emitter.emit(
            MessageType::Error,
            &format!("Preset {} not found", data.preset_id),
        );
        return Ok(());
    };
    if !ctx.app.config.desktop_mode {
        ctx.emitter
            .emit(MessageType::Error, &"File dialog not available");
        return Ok(());
    }

    let request = crate::desktop_dialogs::FileSaveRequest {
        filter_name: "Preset Files",
        filter_extensions: &["json"],
        suggested_file_name: format!("{}.json", data.preset_name),
        initial_directory: None,
    };
    let Some(path) = ctx.app.dialogs.save_file(request).await else {
        ctx.emitter
            .emit(MessageType::NoFileSelected, &"No file selected");
        return Ok(());
    };

    let contents = serde_json::to_string_pretty(preset)?;
    std::fs::write(&path, contents)
        .map_err(|e| HandlerError::Other(format!("Failed to write preset file: {e}")))?;

    ctx.emitter.emit(
        MessageType::ExportPreset,
        &serde_json::json!({
            "message": format!("Preset {} exported successfully", data.preset_name),
            "file_path": path,
        }),
    );
    Ok(())
}

/// Turns a preset name into a safe, unique `<name>.json` zip entry. Path/reserved
/// characters and controls become `_`; blank names become `preset`; collisions
/// get a `-2`, `-3`, … suffix.
fn zip_entry_name(name: &str, used: &mut std::collections::HashSet<String>) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let base = if sanitized.trim().is_empty() {
        "preset".to_string()
    } else {
        sanitized
    };
    let mut candidate = format!("{base}.json");
    let mut n = 2;
    while used.contains(&candidate) {
        candidate = format!("{base}-{n}.json");
        n += 1;
    }
    used.insert(candidate.clone());
    candidate
}

/// Bulk export: writes one `<name>.json` per requested preset into a single zip.
/// Missing preset ids are skipped. Like the single export, requires desktop mode.
pub async fn handle_export_presets(
    data: Vec<ExportPresetData>,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !ctx.app.config.desktop_mode {
        ctx.emitter
            .emit(MessageType::Error, &"File dialog not available");
        return Ok(());
    }
    let presets = psp_db::presets::get_all(&ctx.app.db).await?;

    let request = crate::desktop_dialogs::FileSaveRequest {
        filter_name: "Preset Archives",
        filter_extensions: &["zip"],
        suggested_file_name: "presets.zip".to_string(),
        initial_directory: None,
    };
    let Some(path) = ctx.app.dialogs.save_file(request).await else {
        ctx.emitter
            .emit(MessageType::NoFileSelected, &"No file selected");
        return Ok(());
    };

    use std::io::Write;
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut exported = 0usize;
    {
        let mut zip_writer = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for item in &data {
            let Some(preset) = presets.get(&item.preset_id) else {
                continue;
            };
            let entry_name = zip_entry_name(&item.preset_name, &mut used_names);
            zip_writer
                .start_file(entry_name, options)
                .map_err(|e| HandlerError::Other(e.to_string()))?;
            let contents = serde_json::to_string_pretty(preset)?;
            zip_writer
                .write_all(contents.as_bytes())
                .map_err(|e| HandlerError::Other(format!("Failed to write preset entry: {e}")))?;
            exported += 1;
        }
        zip_writer
            .finish()
            .map_err(|e| HandlerError::Other(e.to_string()))?;
    }
    std::fs::write(&path, cursor.into_inner())
        .map_err(|e| HandlerError::Other(format!("Failed to write preset archive: {e}")))?;

    ctx.emitter.emit(
        MessageType::ExportPresets,
        &serde_json::json!({
            "message": format!("{exported} presets exported successfully"),
            "file_path": path,
        }),
    );
    Ok(())
}

/// Drops the identifiers a freshly imported preset must not keep: the top-level
/// `id` (so `add` mints a new one), the derived `pal_preset_id`, and any nested
/// `pal_preset.id`.
fn strip_preset_ids(preset: &mut serde_json::Value) {
    if let Some(object) = preset.as_object_mut() {
        object.remove("id");
        object.remove("pal_preset_id");
        if let Some(pal_preset) = object.get_mut("pal_preset").and_then(|v| v.as_object_mut()) {
            pal_preset.remove("id");
        }
    }
}

/// Imports every preset in `value` — a single object, or an array of objects —
/// stripping identifiers so each is added with a fresh id. Non-object entries
/// are skipped. Returns the number of presets added.
async fn import_preset_value(
    db: &sqlx::SqlitePool,
    value: serde_json::Value,
) -> Result<usize, HandlerError> {
    let items = match value {
        serde_json::Value::Array(items) => items,
        other => vec![other],
    };
    let mut imported = 0;
    for mut preset in items {
        if !preset.is_object() {
            continue;
        }
        strip_preset_ids(&mut preset);
        psp_db::presets::add(db, preset).await?;
        imported += 1;
    }
    Ok(imported)
}

pub async fn handle_import_preset(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    if !ctx.app.config.desktop_mode {
        ctx.emitter
            .emit(MessageType::Error, &"File dialog not available");
        return Ok(());
    }

    let request = crate::desktop_dialogs::FileDialogRequest {
        filter_name: "Preset Files",
        filter_extensions: &["zip", "json"],
        initial_directory: None,
    };
    let Some(paths) = ctx.app.dialogs.pick_files(request).await else {
        ctx.emitter
            .emit(MessageType::NoFileSelected, &"No file selected");
        return Ok(());
    };
    if paths.is_empty() {
        ctx.emitter
            .emit(MessageType::NoFileSelected, &"No file selected");
        return Ok(());
    }

    let mut imported = 0usize;
    let mut skipped = 0usize;
    for path in &paths {
        let is_zip = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("zip"))
            .unwrap_or(false);
        if is_zip {
            match import_zip(&ctx.app.db, path).await {
                Ok((added, bad)) => {
                    imported += added;
                    skipped += bad;
                }
                Err(_) => skipped += 1,
            }
        } else {
            let parsed = std::fs::read_to_string(path)
                .ok()
                .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok());
            match parsed {
                Some(value) => match import_preset_value(&ctx.app.db, value).await {
                    Ok(added) => imported += added,
                    Err(_) => skipped += 1,
                },
                None => skipped += 1,
            }
        }
    }

    let message = if skipped > 0 {
        format!("{imported} presets imported, {skipped} skipped")
    } else {
        format!("{imported} presets imported successfully")
    };
    ctx.emitter.emit(
        MessageType::ImportPreset,
        &serde_json::json!({ "message": message, "count": imported }),
    );
    Ok(())
}

/// Reads every `.json` entry from a zip archive and imports it. Returns
/// `(imported, skipped)`. Entries are fully read before any async add so no
/// non-`Send` zip handle is held across an await point.
async fn import_zip(
    db: &sqlx::SqlitePool,
    path: &std::path::Path,
) -> Result<(usize, usize), HandlerError> {
    use std::io::Read;
    let bytes = std::fs::read(path)
        .map_err(|e| HandlerError::Other(format!("Failed to read preset archive: {e}")))?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| HandlerError::Other(e.to_string()))?;

    let mut values = Vec::new();
    let mut skipped = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| HandlerError::Other(e.to_string()))?;
        if entry.is_dir() || !entry.name().to_ascii_lowercase().ends_with(".json") {
            continue;
        }
        let mut contents = String::new();
        if entry.read_to_string(&mut contents).is_err() {
            skipped += 1;
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(&contents) {
            Ok(value) => values.push(value),
            Err(_) => skipped += 1,
        }
    }

    let mut imported = 0usize;
    for value in values {
        match import_preset_value(db, value).await {
            Ok(added) => imported += added,
            Err(_) => skipped += 1,
        }
    }
    Ok((imported, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestContext;

    /// Seeds one preset from a real presets.json fixture and checks the
    /// id-keyed dict shape of the response.
    #[tokio::test]
    async fn get_presets_seeds_from_json_and_returns_dict() {
        let mut test = TestContext::new(|json_dir| {
            std::fs::write(
                json_dir.join("presets.json"),
                serde_json::json!([{"name": "Melee", "type": "inventory"}]).to_string(),
            )
            .unwrap();
        })
        .await;
        handle_get_presets(&mut HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            attachment: None,
        })
        .await
        .unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_presets");
        let presets = frame["data"].as_object().expect("dict payload");
        assert_eq!(presets.len(), 1);
        let (_id, preset) = presets.iter().next().unwrap();
        assert_eq!(preset["name"], "Melee");
        assert_eq!(preset["type"], "inventory");

        // populate_from_json only seeds when the table is EMPTY — a second
        // call must not duplicate the seeded row.
        handle_get_presets(&mut HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            attachment: None,
        })
        .await
        .unwrap();
        let second_frame = test.next_frame_json();
        assert_eq!(second_frame["data"].as_object().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn import_preset_value_adds_single_object_and_strips_id() {
        let test = TestContext::new(|_json_dir| {}).await;
        let value = serde_json::json!({
            "id": "old-id", "name": "Solo", "type": "inventory"
        });
        let added = import_preset_value(&test.app.db, value).await.unwrap();
        assert_eq!(added, 1);

        let presets = psp_db::presets::get_all(&test.app.db).await.unwrap();
        assert_eq!(presets.len(), 1);
        let (new_id, preset) = presets.iter().next().unwrap();
        assert_ne!(new_id, "old-id");
        assert_eq!(preset["name"], "Solo");
    }

    #[tokio::test]
    async fn import_preset_value_adds_every_item_in_array() {
        let test = TestContext::new(|_json_dir| {}).await;
        let value = serde_json::json!([
            {"name": "A", "type": "inventory"},
            {"name": "B", "type": "inventory"},
        ]);
        let added = import_preset_value(&test.app.db, value).await.unwrap();
        assert_eq!(added, 2);
        assert_eq!(psp_db::presets::get_all(&test.app.db).await.unwrap().len(), 2);
    }

    #[test]
    fn zip_entry_name_dedupes_and_sanitizes() {
        let mut used = std::collections::HashSet::new();
        assert_eq!(zip_entry_name("Melee", &mut used), "Melee.json");
        assert_eq!(zip_entry_name("Melee", &mut used), "Melee-2.json");
        assert_eq!(zip_entry_name("Melee", &mut used), "Melee-3.json");
        assert_eq!(zip_entry_name("a/b:c", &mut used), "a_b_c.json");
        assert_eq!(zip_entry_name("   ", &mut used), "preset.json");
    }
}
