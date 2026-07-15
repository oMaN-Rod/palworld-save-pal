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

pub async fn handle_import_preset(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    if !ctx.app.config.desktop_mode {
        ctx.emitter
            .emit(MessageType::Error, &"File dialog not available");
        return Ok(());
    }

    let request = crate::desktop_dialogs::FileDialogRequest {
        filter_name: "Preset Files",
        filter_extensions: &["json"],
        initial_directory: None,
    };
    let Some(path) = ctx.app.dialogs.pick_file(request).await else {
        ctx.emitter
            .emit(MessageType::NoFileSelected, &"No file selected");
        return Ok(());
    };

    let contents = std::fs::read_to_string(&path)
        .map_err(|e| HandlerError::Other(format!("Failed to read preset file: {e}")))?;
    let mut preset: serde_json::Value = serde_json::from_str(&contents)?;
    strip_preset_ids(&mut preset);

    let preset_id = psp_db::presets::add(&ctx.app.db, preset).await?;
    ctx.emitter.emit(
        MessageType::ImportPreset,
        &serde_json::json!({
            "message": "Preset imported successfully",
            "preset_id": preset_id,
            "file_path": path,
        }),
    );
    Ok(())
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
}
