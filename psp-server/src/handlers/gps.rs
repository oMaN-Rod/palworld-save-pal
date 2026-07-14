//! GPS (Global Pal Storage) WS handlers: `request_gps`, `add_gps_pal`,
//! `clone_gps_pal`, `delete_gps_pals`, `clone_gps_pal_to_player`.

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use psp_core::domain::pal;
use psp_core::domain::player::build_player_dto;
use psp_core::dto::pal::PalDto;

#[derive(Debug, serde::Deserialize)]
pub struct AddGpsPalData {
    pub character_id: String,
    pub nickname: String,
    pub storage_slot: Option<i32>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CloneGpsPalData {
    pub pal: PalDto,
}

#[derive(Debug, serde::Deserialize)]
pub struct DeleteGpsPalsData {
    pub pal_indexes: Vec<i32>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CloneGpsPalToPlayerData {
    pub pal_ids: Vec<String>,
    pub destination_type: String,
    pub destination_player_uid: String,
}

/// The GPS pals as a JSON object keyed by the slot's decimal string, in
/// ascending slot order — the shape the frontend indexes its slot grid by.
fn gps_map_json(pals: &std::collections::BTreeMap<i32, PalDto>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (slot, pal_dto) in pals {
        map.insert(
            slot.to_string(),
            serde_json::to_value(pal_dto).expect("PalDto serializes"),
        );
    }
    serde_json::Value::Object(map)
}

pub async fn handle_request_gps(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let Some(save) = ctx.session.save.as_mut() else {
        ctx.emitter.emit(
            MessageType::GetGpsResponse,
            &serde_json::json!({"error": "No save file loaded"}),
        );
        return Ok(());
    };
    if save.gps.loaded {
        let payload = gps_map_json(&save.gps.pals);
        ctx.emitter.emit(MessageType::GetGpsResponse, &payload);
        return Ok(());
    }
    let Some(file_path) = save.gps.file_path.clone() else {
        ctx.emitter.emit(
            MessageType::GetGpsResponse,
            &serde_json::json!({"available": false, "message": "No GPS file available for this save"}),
        );
        return Ok(());
    };
    ctx.emitter.emit(
        MessageType::ProgressMessage,
        &"Loading Global Pal Storage...",
    );
    let load_result = std::fs::read(&file_path)
        .map_err(psp_core::error::CoreError::from)
        .and_then(|bytes| save.load_gps(&bytes, &ctx.app.game_data).map(|_| ()));
    match load_result {
        Ok(()) => {
            let payload = gps_map_json(&save.gps.pals);
            ctx.emitter.emit(MessageType::GetGpsResponse, &payload);
        }
        Err(error) => {
            tracing::error!(%error, "failed to load GPS");
            ctx.emitter.emit(
                MessageType::GetGpsResponse,
                &serde_json::json!({"available": false, "message": "No GPS file available for this save"}),
            );
        }
    }
    Ok(())
}

pub async fn handle_add_gps_pal(
    data: AddGpsPalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(save) = ctx.session.save.as_mut() else {
        return Ok(()); // no save: answer nothing
    };
    match save.add_gps_pal(
        &ctx.app.game_data,
        &data.character_id,
        &data.nickname,
        data.storage_slot,
    )? {
        Some((new_pal, slot_index)) => ctx.emitter.emit(
            MessageType::AddGpsPal,
            &serde_json::json!({"pal": new_pal, "index": slot_index}),
        ),
        None => ctx.emitter.emit(
            MessageType::AddGpsPal,
            &serde_json::json!({"error": "Failed to add pal. No available slots or invalid data."}),
        ),
    }
    Ok(())
}

pub async fn handle_clone_gps_pal(
    data: CloneGpsPalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(save) = ctx.session.save.as_mut() else {
        return Ok(()); // no save: answer nothing
    };
    match save.add_gps_pal_from_dto(&ctx.app.game_data, &data.pal, None)? {
        // A clone answers under `add_gps_pal`, not `clone_gps_pal` — on both
        // the success and the failure branch. The frontend listens for that.
        Some((slot_index, new_pal)) => ctx.emitter.emit(
            MessageType::AddGpsPal,
            &serde_json::json!({"pal": new_pal, "index": slot_index}),
        ),
        None => ctx.emitter.emit(
            MessageType::AddGpsPal,
            &serde_json::json!({"error": "Failed to clone pal. No available slots."}),
        ),
    }
    Ok(())
}

pub async fn handle_delete_gps_pals(
    data: DeleteGpsPalsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(save) = ctx.session.save.as_mut() else {
        return Ok(());
    };
    save.delete_gps_pals(&data.pal_indexes);
    Ok(()) // deliberately silent: the frontend does not await a response here
}

pub async fn handle_clone_gps_pal_to_player(
    data: CloneGpsPalToPlayerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if ctx.session.save.is_none() {
        ctx.emitter.emit(
            MessageType::Error,
            &serde_json::json!({"message": "No save file loaded"}),
        );
        return Ok(());
    }
    if data.destination_type != "pal_box" && data.destination_type != "dps" {
        ctx.emitter.emit(
            MessageType::Error,
            &serde_json::json!({"message": format!("Invalid destination type: {}", data.destination_type)}),
        );
        return Ok(());
    }
    let Ok(player_uid) = uuid::Uuid::parse_str(&data.destination_player_uid) else {
        ctx.emitter.emit(
            MessageType::Error,
            &serde_json::json!({"message": "Player not found"}),
        );
        return Ok(());
    };

    // Players are loaded lazily, so `build_player_dto` alone would reject a
    // real-but-not-yet-opened destination player. Check existence against the
    // eagerly built `player_summaries` first, then force-load the player's
    // GVAS. Same guard as `handlers::ups::handle_export_ups_pal`.
    let save = ctx.session.save.as_ref().unwrap();
    if !save.player_summaries.contains_key(&player_uid) {
        ctx.emitter.emit(
            MessageType::Error,
            &serde_json::json!({"message": "Player not found"}),
        );
        return Ok(());
    }
    let save = ctx.session.save_mut()?;
    save.ensure_player_loaded(player_uid)?;
    let save = ctx.session.save.as_ref().unwrap();
    let Some(player) = build_player_dto(save, &ctx.app.game_data, player_uid)? else {
        ctx.emitter.emit(
            MessageType::Error,
            &serde_json::json!({"message": "Player not found"}),
        );
        return Ok(());
    };
    // Resolved but deliberately NOT guarded here: a missing pal box must not
    // reject a "dps"-destination request. The `None` case becomes a per-pal
    // failure inside the pal_box branch below.
    let pal_box_id = player.pal_box_id;
    if save.gps_pals().map(|pals| pals.is_empty()).unwrap_or(true) {
        ctx.emitter.emit(
            MessageType::Error,
            &serde_json::json!({"message": "GPS not available"}),
        );
        return Ok(());
    }

    let mut cloned_count = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for pal_id_text in &data.pal_ids {
        // Clone the source `PalDto` out from under the immutable borrow before
        // the mutable `add_player_*_from_dto` call below; holding it across
        // would not compile.
        let save = ctx.session.save.as_ref().unwrap();
        let source_dto: Option<PalDto> =
            uuid::Uuid::parse_str(pal_id_text).ok().and_then(|pal_id| {
                save.gps_pals()
                    .and_then(|pals| pals.values().find(|dto| dto.instance_id == pal_id))
                    .cloned()
            });
        let Some(pal_dto) = source_dto else {
            errors.push(format!("Pal not found in GPS: {pal_id_text}"));
            continue;
        };

        let save = ctx.session.save.as_mut().unwrap();
        if data.destination_type == "pal_box" {
            // Per-pal failure, NOT a request-level `error` frame.
            let Some(pal_box_id) = pal_box_id else {
                errors.push(format!("Failed to add pal to pal box: {pal_id_text}"));
                continue;
            };
            match pal::add_player_pal_from_dto(
                save,
                &ctx.app.game_data,
                player_uid,
                &pal_dto,
                pal_box_id,
                None,
            )? {
                Some(new_pal) => {
                    ctx.emitter.emit(
                        MessageType::AddPal,
                        &serde_json::json!({"player_id": player_uid.to_string(), "pal": new_pal}),
                    );
                    cloned_count += 1;
                }
                None => errors.push(format!("Failed to add pal to pal box: {pal_id_text}")),
            }
        } else {
            match pal::add_player_dps_pal_from_dto(
                save,
                &ctx.app.game_data,
                player_uid,
                &pal_dto,
                None,
            )? {
                Some((slot_index, new_pal)) => {
                    ctx.emitter.emit(
                        MessageType::AddDpsPal,
                        &serde_json::json!({
                            "player_id": player_uid.to_string(),
                            "pal": new_pal,
                            "index": slot_index,
                        }),
                    );
                    cloned_count += 1;
                }
                None => errors.push(format!("Failed to add pal to DPS: {pal_id_text}")),
            }
        }
    }

    ctx.emitter.emit(
        MessageType::CloneGpsPalToPlayer,
        &serde_json::json!({
            "success": cloned_count > 0,
            "cloned_count": cloned_count,
            "errors": errors,
        }),
    );
    Ok(())
}
