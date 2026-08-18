//! Blueprint capture/place WS handlers: captures a base out of the loaded
//! save into the connection's `BlueprintRegistry`, and (in later tasks)
//! validates and places a captured blueprint back into a save.

use uuid::Uuid;

use psp_core::domain::blueprint::validate::{Anchor, Finding, PlacementMode, Severity};
use psp_core::domain::blueprint::{capture, BlueprintStructure, CaptureOptions};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

#[derive(Debug, serde::Deserialize)]
pub struct CaptureBlueprintData {
    pub base_id: Uuid,
    pub options: CaptureOptions,
    pub name: String,
}

/// Every consumer message carries just a handle.
#[derive(Debug, serde::Deserialize)]
pub struct HandleData {
    pub handle: Uuid,
}

/// The frontend resolves terrain and sends a fully-resolved absolute anchor.
#[derive(Debug, serde::Deserialize)]
pub struct AnchorDto {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f64,
}

impl AnchorDto {
    pub fn to_anchor(&self) -> Anchor {
        Anchor {
            x: self.x,
            y: self.y,
            z: self.z,
            yaw_radians: self.yaw,
        }
    }
}

/// `Finding` has no serde derive; the wire form is this small object.
pub fn finding_json(finding: &Finding) -> serde_json::Value {
    let severity = match finding.severity {
        Severity::Blocking => "blocking",
        Severity::Warning => "warning",
    };
    serde_json::json!({ "severity": severity, "code": finding.code, "message": finding.message })
}

/// Turns the wire `mode` + optional target ids into a `PlacementMode`.
pub fn resolve_mode(
    mode: &str,
    target_guild: Option<Uuid>,
    target_base: Option<Uuid>,
) -> Result<PlacementMode, HandlerError> {
    match mode {
        "new_base" => target_guild
            .map(|guild_id| PlacementMode::NewBase { guild_id })
            .ok_or_else(|| HandlerError::Other("new_base placement requires target_guild".into())),
        "merge_into" => target_base
            .map(|base_id| PlacementMode::MergeInto { base_id })
            .ok_or_else(|| HandlerError::Other("merge_into placement requires target_base".into())),
        other => Err(HandlerError::Other(format!(
            "unknown placement mode {other}"
        ))),
    }
}

pub async fn handle_capture_base_blueprint(
    data: CaptureBlueprintData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(session) = ctx.session.save.as_ref() else {
        return Err(HandlerError::Other("No save file loaded".to_string()));
    };
    let mut blueprint = capture::capture(session, data.base_id, data.options, &data.name)?;
    blueprint.header.created_at = chrono::Utc::now().timestamp();
    blueprint.header.game_data_version = ctx.app.game_data.version().to_string();

    let header = serde_json::to_value(&blueprint.header)?;
    let handle = ctx.blueprints.insert(blueprint);
    ctx.emitter.emit(
        MessageType::CaptureBaseBlueprint,
        &serde_json::json!({ "handle": handle, "header": header }),
    );
    Ok(())
}

pub async fn handle_store_blueprint(
    data: HandleData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let row = {
        let Some(blueprint) = ctx.blueprints.get(&data.handle) else {
            return Err(HandlerError::Other(format!(
                "Unknown blueprint handle {}",
                data.handle
            )));
        };
        let header = &blueprint.header;
        psp_db::blueprints::NewBlueprint {
            id: None,
            name: header.name.clone(),
            source_world: header.source_world.clone(),
            source_base: header.source_base.clone(),
            created_at: header.created_at,
            schema_version: header.schema_version as i64,
            structure_count: header.structure_count as i64,
            manifest: serde_json::to_string(&header.manifest)?,
            footprint_radius: header.footprint_radius,
            payload: psp_core::domain::blueprint::gvas::to_psp_bytes(blueprint)?,
            preview: None,
        }
    };
    let id = psp_db::blueprints::insert(&*ctx.app.driver, row).await?;
    ctx.emitter.emit(
        MessageType::StoreBlueprint,
        &serde_json::json!({ "id": id }),
    );
    Ok(())
}

pub async fn handle_list_blueprints(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let blueprints = psp_db::blueprints::list(&*ctx.app.driver).await?;
    ctx.emitter.emit(
        MessageType::ListBlueprints,
        &serde_json::json!({ "blueprints": blueprints }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct LoadBlueprintData {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
}

pub async fn handle_load_blueprint(
    data: LoadBlueprintData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    use base64::Engine as _;
    use psp_core::domain::blueprint::gvas;

    let blueprint = if let Some(id) = &data.id {
        let stored = psp_db::blueprints::get(&*ctx.app.driver, id)
            .await?
            .ok_or_else(|| HandlerError::Other(format!("Blueprint {id} not found")))?;
        gvas::from_psp_bytes(&stored.payload)?
    } else if let Some(content) = &data.content {
        let format = data.format.as_deref().unwrap_or("psp");
        match format {
            "json" => {
                let text = String::from_utf8(
                    base64::engine::general_purpose::STANDARD
                        .decode(content)
                        .map_err(|e| HandlerError::Other(format!("invalid base64: {e}")))?,
                )
                .map_err(|e| {
                    HandlerError::Other(format!("invalid utf-8 in json blueprint: {e}"))
                })?;
                gvas::from_json(&text)?
            }
            _ => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(content)
                    .map_err(|e| HandlerError::Other(format!("invalid base64: {e}")))?;
                gvas::from_psp_bytes(&bytes)?
            }
        }
    } else {
        return Err(HandlerError::Other(
            "load_blueprint requires either id or content".to_string(),
        ));
    };

    let header = serde_json::to_value(&blueprint.header)?;
    let handle = ctx.blueprints.insert(blueprint);
    ctx.emitter.emit(
        MessageType::LoadBlueprint,
        &serde_json::json!({ "handle": handle, "header": header }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct ExportBlueprintData {
    pub handle: Uuid,
    pub format: String,
}

pub async fn handle_export_blueprint_file(
    data: ExportBlueprintData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    use base64::Engine as _;
    use psp_core::domain::blueprint::gvas;

    let (bytes, extension) = {
        let Some(blueprint) = ctx.blueprints.get(&data.handle) else {
            return Err(HandlerError::Other(format!(
                "Unknown blueprint handle {}",
                data.handle
            )));
        };
        match data.format.as_str() {
            "json" => (gvas::to_json(blueprint)?.into_bytes(), "json"),
            "psp" => (gvas::to_psp_bytes(blueprint)?, "psp"),
            other => {
                return Err(HandlerError::Other(format!(
                    "unknown export format {other}"
                )))
            }
        }
    };

    let name = ctx
        .blueprints
        .get(&data.handle)
        .map(|bp| bp.header.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "blueprint".to_string());
    let file_name = format!("{name}.{extension}");

    if ctx.app.config.desktop_mode {
        let filter_extensions: &'static [&'static str] = match extension {
            "json" => &["json"],
            _ => &["psp"],
        };
        let request = crate::desktop_dialogs::FileSaveRequest {
            filter_name: "Blueprint Files",
            filter_extensions,
            suggested_file_name: file_name.clone(),
            initial_directory: None,
        };
        let Some(path) = ctx.app.dialogs.save_file(request).await else {
            ctx.emitter
                .emit(MessageType::NoFileSelected, &"No file selected");
            return Ok(());
        };
        std::fs::write(&path, &bytes)
            .map_err(|e| HandlerError::Other(format!("Failed to write blueprint file: {e}")))?;
        ctx.emitter.emit(
            MessageType::ExportBlueprintFile,
            &serde_json::json!({
                "message": format!("Blueprint {name} exported successfully"),
                "file_path": path,
            }),
        );
    } else {
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        ctx.emitter.emit(
            MessageType::ExportBlueprintFile,
            &serde_json::json!([{ "name": file_name, "content": encoded }]),
        );
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct PlacementQueryData {
    pub handle: Uuid,
    pub anchor: AnchorDto,
    pub mode: String,
    #[serde(default)]
    pub target_guild: Option<Uuid>,
    #[serde(default)]
    pub target_base: Option<Uuid>,
}

pub async fn handle_validate_blueprint_placement(
    data: PlacementQueryData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    use psp_core::domain::blueprint::validate;

    let Some(blueprint) = ctx.blueprints.get(&data.handle) else {
        return Err(HandlerError::Other(format!(
            "Unknown blueprint handle {}",
            data.handle
        )));
    };
    let Some(session) = ctx.session.save.as_ref() else {
        return Err(HandlerError::Other("No save file loaded".to_string()));
    };
    let mode = resolve_mode(&data.mode, data.target_guild, data.target_base)?;
    let anchor = data.anchor.to_anchor();

    let findings = validate::check(session, &ctx.app.game_data, blueprint, &anchor, &mode);
    let has_blocking = validate::has_blocking(&findings);
    let findings_json: Vec<serde_json::Value> = findings.iter().map(finding_json).collect();
    ctx.emitter.emit(
        MessageType::ValidateBlueprintPlacement,
        &serde_json::json!({ "findings": findings_json, "has_blocking": has_blocking }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct PlaceBlueprintData {
    pub handle: Uuid,
    pub anchor: AnchorDto,
    pub mode: String,
    pub target_player: Uuid,
    #[serde(default)]
    pub target_guild: Option<Uuid>,
    #[serde(default)]
    pub target_base: Option<Uuid>,
    pub override_warnings: bool,
}

pub async fn handle_place_blueprint(
    data: PlaceBlueprintData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    use psp_core::domain::blueprint::place::{self, PlacementRequest};

    let mode = resolve_mode(&data.mode, data.target_guild, data.target_base)?;
    let request = PlacementRequest {
        anchor: data.anchor.to_anchor(),
        mode,
        owner_player_uid: data.target_player,
        override_warnings: data.override_warnings,
    };

    // Clone the blueprint out of the registry so the mutable session borrow
    // below does not overlap the registry borrow.
    let blueprint =
        ctx.blueprints.get(&data.handle).cloned().ok_or_else(|| {
            HandlerError::Other(format!("Unknown blueprint handle {}", data.handle))
        })?;

    let game_data = std::sync::Arc::clone(&ctx.app.game_data);
    let session = ctx.session.save_mut()?;
    let result = place::place(session, &blueprint, &request, &game_data)?;

    let findings: Vec<serde_json::Value> = result.findings.iter().map(finding_json).collect();
    ctx.emitter.emit(
        MessageType::PlaceBlueprint,
        &serde_json::json!({
            "base_id": result.base_id,
            "structures_placed": result.structures_placed,
            "findings": findings,
        }),
    );
    Ok(())
}

/// `BlueprintStructure` has no serde derive; the wire form is this small object.
fn structure_geometry_json(structure: &BlueprintStructure) -> serde_json::Value {
    let t = &structure.relative_transform;
    serde_json::json!({
        "map_object_id": structure.map_object_id,
        "translation": { "x": t.translation.x.0, "y": t.translation.y.0, "z": t.translation.z.0 },
        "rotation": {
            "x": t.rotation.x.0, "y": t.rotation.y.0, "z": t.rotation.z.0, "w": t.rotation.w.0
        },
        "scale": { "x": t.scale.x.0, "y": t.scale.y.0, "z": t.scale.z.0 },
    })
}

pub async fn handle_request_blueprint_geometry(
    data: HandleData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(blueprint) = ctx.blueprints.get(&data.handle) else {
        return Err(HandlerError::Other(format!(
            "Unknown blueprint handle {}",
            data.handle
        )));
    };
    let structures: Vec<serde_json::Value> = blueprint
        .structures
        .iter()
        .map(structure_geometry_json)
        .collect();
    let origin = psp_core::domain::blueprint::capture::source_origin(blueprint)
        .map(|(x, y, z, yaw)| serde_json::json!({ "x": x, "y": y, "z": z, "yaw": yaw }))
        .unwrap_or_else(|| serde_json::json!({ "x": 0.0, "y": 0.0, "z": 0.0, "yaw": 0.0 }));
    ctx.emitter.emit(
        MessageType::RequestBlueprintGeometry,
        &serde_json::json!({ "structures": structures, "origin": origin }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct DeleteBlueprintData {
    pub id: String,
}

pub async fn handle_delete_blueprint(
    data: DeleteBlueprintData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    psp_db::blueprints::delete(&*ctx.app.driver, &data.id).await?;
    ctx.emitter.emit(
        MessageType::DeleteBlueprint,
        &serde_json::json!({ "id": data.id }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(severity: Severity, code: &str) -> Finding {
        Finding {
            severity,
            code: code.to_string(),
            message: format!("{code} tripped"),
        }
    }

    #[test]
    fn finding_json_labels_each_severity_and_carries_code_and_message() {
        let blocking = finding_json(&finding(Severity::Blocking, "base_limit"));
        assert_eq!(blocking["severity"], "blocking");
        assert_eq!(blocking["code"], "base_limit");
        assert_eq!(blocking["message"], "base_limit tripped");

        let warning = finding_json(&finding(Severity::Warning, "unknown_structure_type"));
        assert_eq!(warning["severity"], "warning");
        assert_eq!(warning["code"], "unknown_structure_type");

        // The severity label is what tells the frontend whether a finding can be
        // overridden, so Warning must never serialize to the Blocking string.
        assert_ne!(warning["severity"], blocking["severity"]);
    }

    #[test]
    fn resolve_mode_maps_each_wire_mode_and_rejects_a_missing_target() {
        let guild = Uuid::new_v4();
        let base = Uuid::new_v4();

        match resolve_mode("new_base", Some(guild), None) {
            Ok(PlacementMode::NewBase { guild_id }) => assert_eq!(guild_id, guild),
            other => panic!("new_base with a guild should resolve to NewBase, got {other:?}"),
        }
        match resolve_mode("merge_into", None, Some(base)) {
            Ok(PlacementMode::MergeInto { base_id }) => assert_eq!(base_id, base),
            other => panic!("merge_into with a base should resolve to MergeInto, got {other:?}"),
        }

        // A new_base naming no guild, and a merge_into naming no base, must both
        // be refused rather than silently borrowing the other mode's target or
        // defaulting to a nil id — the presence of the wrong target must not save it.
        assert!(
            resolve_mode("new_base", None, Some(base)).is_err(),
            "new_base without target_guild is refused even when a base is present"
        );
        assert!(
            resolve_mode("merge_into", Some(guild), None).is_err(),
            "merge_into without target_base is refused even when a guild is present"
        );
        assert!(
            resolve_mode("teleport", Some(guild), Some(base)).is_err(),
            "an unknown mode string is refused"
        );
    }
}
