//! Web-worker save flow (M1): the browser decompresses `.sav` to raw GVAS
//! before handing it to the engine, and re-compresses on download. These two
//! handlers ingest/emit raw GVAS bundles instead of `.sav`/zip bytes.

use std::collections::BTreeMap;

use base64::Engine;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use psp_core::session::{PlayerFileData, SaveKind, SaveSession};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::handlers::save_file::emit_loaded_save;
use crate::messages::MessageType;

#[derive(Debug, Deserialize)]
pub struct GvasPlayer {
    pub uid: String,
    /// base64 raw GVAS
    pub sav: String,
    pub dps: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoadSaveGvasPayload {
    pub save_id: String,
    pub level: String,
    pub level_meta: Option<String>,
    pub world_option: Option<String>,
    pub players: Vec<GvasPlayer>,
}

fn b64(input: &str) -> Result<Vec<u8>, HandlerError> {
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|error| HandlerError::Other(format!("invalid base64: {error}")))
}

pub async fn handle_load_save_gvas(
    payload: LoadSaveGvasPayload,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let level = b64(&payload.level)?;
    let level_meta = payload.level_meta.as_deref().map(b64).transpose()?;
    let world_option = payload.world_option.as_deref().map(b64).transpose()?;

    let mut player_order: Vec<Uuid> = Vec::new();
    let mut player_file_refs: BTreeMap<Uuid, PlayerFileData> = BTreeMap::new();
    for player in &payload.players {
        let Ok(uid) = player.uid.parse::<Uuid>() else {
            return Err(HandlerError::Other(format!("invalid player uid: {}", player.uid)));
        };
        let sav = b64(&player.sav)?;
        let dps = player.dps.as_deref().map(b64).transpose()?;
        if !player_file_refs.contains_key(&uid) {
            player_order.push(uid);
        }
        player_file_refs.insert(uid, PlayerFileData::Bytes { sav: Some(sav), dps });
    }

    let progress = ctx.emitter.progress_sink();
    let session = SaveSession::load(
        SaveKind::InMemory,
        payload.save_id,
        "steam",
        &level,
        level_meta.as_deref(),
        world_option.as_deref(),
        player_file_refs,
        None, // GPS deferred within M1
        true,
        &progress,
    )?;
    emit_loaded_save(ctx, session, player_order, false)
}

#[derive(Debug, Serialize)]
struct GvasPlayerOut {
    uid: String,
    sav: String,
    dps: Option<String>,
}

#[derive(Debug, Serialize)]
struct SaveGvasBundle {
    world_name: String,
    level: String,
    level_meta: Option<String>,
    world_option: Option<String>,
    players: Vec<GvasPlayerOut>,
}

fn enc(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub async fn handle_download_save_gvas(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let Some(session) = ctx.session.save.as_ref() else {
        return Err(HandlerError::Other("No save file loaded".to_string()));
    };
    let bundle = SaveGvasBundle {
        world_name: session.world_name.clone(),
        level: enc(&session.level_gvas_bytes()?),
        level_meta: session.level_meta_gvas_bytes()?.map(|b| enc(&b)),
        world_option: session.world_option_gvas_bytes()?.map(|b| enc(&b)),
        players: session
            .player_gvas_bytes()?
            .into_iter()
            .map(|(uid, (sav, dps))| GvasPlayerOut {
                uid: uid.to_string(),
                sav: enc(&sav),
                dps: dps.map(|b| enc(&b)),
            })
            .collect(),
    };
    ctx.emitter.emit(MessageType::SaveGvasBundle, &bundle);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestContext;
    use base64::Engine;

    /// `SaveSession::load` emits its own `progress_message` frames (loading /
    /// summary-extraction narration) ahead of the load tail; skip past them to
    /// reach the frame under test.
    fn next_non_progress_frame(test: &mut TestContext) -> serde_json::Value {
        loop {
            let frame = test.next_frame_json();
            if frame["type"] != "progress_message" {
                return frame;
            }
        }
    }

    fn world1_level_gvas() -> String {
        let sav = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/saves/world1/Level.sav"
        ))
        .expect("read world1");
        let save = psp_core::savio::read_sav_bytes(&sav).expect("parse");
        let gvas = psp_core::savio::write_gvas_bytes(&save).expect("gvas");
        base64::engine::general_purpose::STANDARD.encode(gvas)
    }

    #[tokio::test]
    async fn load_gvas_then_download_gvas_round_trips() {
        let mut test = TestContext::new(|_| {}).await;
        let level_b64 = world1_level_gvas();

        let mut arc = std::sync::Arc::new(tokio::sync::Mutex::new(psp_core::session::Session::new()));
        let mut current_id: Option<Uuid> = None;
        {
            let mut ctx = HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
                blueprints: &mut test.blueprints,
                attachment: Some(crate::dispatcher::SessionAttachment {
                    current_id: &mut current_id,
                    arc: &mut arc,
                }),
            };
            handle_load_save_gvas(
                LoadSaveGvasPayload {
                    save_id: "world1".to_string(),
                    level: level_b64.clone(),
                    level_meta: None,
                    world_option: None,
                    players: Vec::new(),
                },
                &mut ctx,
            )
            .await
            .expect("load");
        }
        // loaded_save_files + get_player_summaries + get_guild_summaries emitted,
        // interleaved with the load's own progress_message frames.
        assert_eq!(next_non_progress_frame(&mut test)["type"], "loaded_save_files");
        assert_eq!(next_non_progress_frame(&mut test)["type"], "get_player_summaries");
        assert_eq!(next_non_progress_frame(&mut test)["type"], "get_guild_summaries");

        {
            let mut ctx = HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
                blueprints: &mut test.blueprints,
                attachment: None,
            };
            handle_download_save_gvas(&mut ctx).await.expect("download");
        }
        let frame = next_non_progress_frame(&mut test);
        assert_eq!(frame["type"], "save_gvas_bundle");
        // The emitted level GVAS re-parses and matches the loaded GVAS.
        let out = frame["data"]["level"].as_str().unwrap();
        let out_bytes = base64::engine::general_purpose::STANDARD.decode(out).unwrap();
        assert_eq!(&out_bytes[0..4], b"GVAS");
        let in_bytes = base64::engine::general_purpose::STANDARD.decode(&level_b64).unwrap();
        assert_eq!(out_bytes, in_bytes, "download GVAS equals loaded GVAS");
    }
}
