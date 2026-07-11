//! GPS (Global Pal Storage) WS handlers (Task 3D-2), wiring the 3D-1 session
//! API (`psp_core::domain::gps`'s `impl SaveSession` block) and 3C-6's
//! `build_player_dto`/`add_player_pal_from_dto`/`add_player_dps_pal_from_dto`
//! to the five wire types `request_gps`/`add_gps_pal`/`clone_gps_pal`/
//! `delete_gps_pals`/`clone_gps_pal_to_player`. Wire shapes ported verbatim
//! from `ws/handlers/gps_handler.py` — see this task's report for the full
//! reconciliation against the task brief, which assumed a since-superseded
//! `Pal`/`players()` API that does not exist in this port's actual
//! (DTO/functional) architecture.

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

/// `save.gps.pals` (`BTreeMap<i32, PalDto>`) serialized as the JSON object
/// Python's `Dict[int, Pal]` produces: keyed by the slot's decimal string,
/// ascending order (`BTreeMap`'s natural iteration order already matches).
/// Each `PalDto` is already the wire shape (no `.to_dto()` -- unlike the 3C
/// UPS/pal handlers' `Pal` domain objects, GPS stores `PalDto`s directly).
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
        return Ok(()); // gps_handler.py:64-65 -- silent
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
        return Ok(()); // silent, gps_handler.py:88-89
    };
    match save.add_gps_pal_from_dto(&ctx.app.game_data, &data.pal, None)? {
        // Success and failure both use the ADD_GPS_PAL type (gps_handler.py:94-103).
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
        return Ok(()); // silent, gps_handler.py:110-113
    };
    save.delete_gps_pals(&data.pal_indexes);
    Ok(()) // never responds (gps_handler.py:106-114)
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

    let save = ctx.session.save.as_ref().unwrap();
    let Some(player) = build_player_dto(save, &ctx.app.game_data, player_uid)? else {
        ctx.emitter.emit(
            MessageType::Error,
            &serde_json::json!({"message": "Player not found"}),
        );
        return Ok(());
    };
    let Some(pal_box_id) = player.pal_box_id else {
        ctx.emitter.emit(
            MessageType::Error,
            &serde_json::json!({"message": "Player not found"}),
        );
        return Ok(());
    };
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
        // Clone the source `PalDto` out of `save.gps_pals()` (immutable
        // borrow) BEFORE calling the mutable `add_player_*_from_dto` below --
        // holding the borrow across the mutable call would not compile (see
        // this task's report / `resolve_source_pal` in ups.rs for the same
        // pattern).
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
