use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

#[derive(Debug, serde::Deserialize)]
pub struct UpdatePresetData {
    pub id: String,
    pub name: String,
}

/// The `data/json/presets.json` seed array (Python's `default_factory=uuid4`
/// equivalent — entries carry no `id`, so `add` generates one per entry).
fn presets_seed(ctx: &HandlerCtx<'_>) -> serde_json::Value {
    ctx.app
        .game_data
        .get("presets")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]))
}

/// Mirrors preset_handler.py:40-46: seed the table from the JSON fixture only
/// when it is empty, then return every preset as an id-keyed object.
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
        // preset_handler.py:60-62 — error data is a plain string here.
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

/// Mirrors preset_handler.py:91-147: both export/import require a native file
/// dialog (`app_state.webview_window`), which only exists in desktop mode. In
/// web mode Python always answers `error` with the plain string
/// "File dialog not available". Phase 3 implements exactly that non-desktop
/// path; Phase 5 wires the native dialog behind `desktop_mode`.
pub async fn handle_export_preset(
    data: ExportPresetData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let presets = psp_db::presets::get_all(&ctx.app.db).await?;
    if !presets.contains_key(&data.preset_id) {
        ctx.emitter.emit(
            MessageType::Error,
            &format!("Preset {} not found", data.preset_id),
        );
        return Ok(());
    }
    if ctx.app.config.desktop_mode {
        // Phase 5 wires the native save dialog here (preset_handler.py:117-147).
    }
    ctx.emitter
        .emit(MessageType::Error, &"File dialog not available");
    Ok(())
}

pub async fn handle_import_preset(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    if ctx.app.config.desktop_mode {
        // Phase 5 wires the native open dialog + id-stripping import here
        // (preset_handler.py:166-237).
    }
    ctx.emitter
        .emit(MessageType::Error, &"File dialog not available");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestContext;

    /// Real (non-stopgap) coverage for handle_get_presets: TestContext gives a
    /// migrated DB pool and a GameData over a temp data/json dir, so this
    /// seeds one preset via a real presets.json fixture and checks the DICT
    /// shape (id -> preset) that replaces the Phase-0 empty-list stopgap.
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
        })
        .await
        .unwrap();
        let second_frame = test.next_frame_json();
        assert_eq!(second_frame["data"].as_object().unwrap().len(), 1);
    }
}
