//! Web-worker save flow: decompressed GVAS in on load, decompressed GVAS out on
//! download; the worker owns (de)compression. Everything here moves raw bytes
//! rather than `.sav`/zip bytes, one file at a time — a whole save base64'd
//! into a single JSON frame runs past the longest string a browser can hold.

use std::collections::BTreeMap;

use uuid::Uuid;

use psp_core::session::{PlayerFileData, SaveKind, SaveSession};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::handlers::save_file::{download_player_stem, emit_loaded_save};

#[derive(Debug)]
pub struct GvasPlayerBytes {
    pub uid: Uuid,
    pub sav: Vec<u8>,
    pub dps: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct LoadSaveGvasBytes {
    pub save_id: String,
    pub level: Vec<u8>,
    pub level_meta: Option<Vec<u8>>,
    pub world_option: Option<Vec<u8>>,
    pub players: Vec<GvasPlayerBytes>,
}

#[derive(Debug)]
struct StagedPlayer {
    uid: Uuid,
    sav: Option<Vec<u8>>,
    dps: Option<Vec<u8>>,
}

#[derive(Debug, Default)]
pub struct StagedGvas {
    level: Option<Vec<u8>>,
    level_meta: Option<Vec<u8>>,
    world_option: Option<Vec<u8>>,
    players: Vec<StagedPlayer>,
}

impl StagedGvas {
    /// `uid` is ignored for the world-level slots.
    pub fn stage(&mut self, slot: &str, uid: &str, bytes: Vec<u8>) -> Result<(), HandlerError> {
        match slot {
            "level" => self.level = Some(bytes),
            "level_meta" => self.level_meta = Some(bytes),
            "world_option" => self.world_option = Some(bytes),
            "player_sav" | "player_dps" => {
                let uid = uid
                    .parse::<Uuid>()
                    .map_err(|_| HandlerError::Other(format!("invalid player uid: {uid}")))?;
                let index = match self.players.iter().position(|p| p.uid == uid) {
                    Some(index) => index,
                    None => {
                        self.players.push(StagedPlayer { uid, sav: None, dps: None });
                        self.players.len() - 1
                    }
                };
                let player = &mut self.players[index];
                if slot == "player_sav" {
                    player.sav = Some(bytes);
                } else {
                    player.dps = Some(bytes);
                }
            }
            other => return Err(HandlerError::Other(format!("unknown GVAS slot: {other}"))),
        }
        Ok(())
    }

    /// Leaves the staging area empty so a second load cannot resurrect the
    /// previous save's buffers.
    pub fn take(&mut self, save_id: String) -> Result<LoadSaveGvasBytes, HandlerError> {
        let level = self
            .level
            .take()
            .ok_or_else(|| HandlerError::Other("no Level.sav was staged".to_string()))?;
        let players = std::mem::take(&mut self.players)
            .into_iter()
            // A `_dps.sav` with no `.sav` beside it is not a player.
            .filter_map(|p| {
                Some(GvasPlayerBytes { uid: p.uid, sav: p.sav?, dps: p.dps })
            })
            .collect();
        Ok(LoadSaveGvasBytes {
            save_id,
            level,
            level_meta: self.level_meta.take(),
            world_option: self.world_option.take(),
            players,
        })
    }
}

pub async fn handle_load_save_gvas_bytes(
    payload: LoadSaveGvasBytes,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let mut player_order: Vec<Uuid> = Vec::new();
    let mut player_file_refs: BTreeMap<Uuid, PlayerFileData> = BTreeMap::new();
    for player in payload.players {
        player_order.push(player.uid);
        player_file_refs.insert(
            player.uid,
            PlayerFileData::Bytes { sav: Some(player.sav), dps: player.dps },
        );
    }

    let progress = ctx.emitter.progress_sink();
    let session = SaveSession::load(
        SaveKind::InMemory,
        payload.save_id,
        "steam",
        &payload.level,
        payload.level_meta.as_deref(),
        payload.world_option.as_deref(),
        player_file_refs,
        None, // GPS is not yet wired for the web GVAS load path
        true,
        &progress,
    )?;
    emit_loaded_save(ctx, session, player_order, false)
}

/// The download zip's file list, without any of its bytes. The caller fetches
/// each entry separately through `export_file`, so only one file's worth of
/// GVAS is ever alive at once.
#[derive(Debug)]
pub struct ExportManifest {
    pub world_name: String,
    pub names: Vec<String>,
}

const LEVEL: &str = "Level.sav";
const LEVEL_META: &str = "LevelMeta.sav";
const WORLD_OPTION: &str = "WorldOption.sav";

pub fn export_manifest(session: &SaveSession) -> ExportManifest {
    let mut names = vec![LEVEL.to_string()];
    if session.level_meta.is_some() {
        names.push(LEVEL_META.to_string());
    }
    if session.world_option.is_some() {
        names.push(WORLD_OPTION.to_string());
    }
    for (uid, loaded) in &session.loaded_players {
        let stem = download_player_stem(uid);
        names.push(format!("Players/{stem}.sav"));
        if loaded.dps.is_some() {
            names.push(format!("Players/{stem}_dps.sav"));
        }
    }
    ExportManifest { world_name: session.world_name.clone(), names }
}

pub fn export_file(session: &SaveSession, name: &str) -> Result<Vec<u8>, HandlerError> {
    let missing = |what: &str| HandlerError::Other(format!("{what} is not loaded"));
    match name {
        LEVEL => Ok(session.level_gvas_bytes()?),
        LEVEL_META => session.level_meta_gvas_bytes()?.ok_or_else(|| missing(LEVEL_META)),
        WORLD_OPTION => session.world_option_gvas_bytes()?.ok_or_else(|| missing(WORLD_OPTION)),
        _ => {
            let stem = name
                .strip_prefix("Players/")
                .and_then(|rest| rest.strip_suffix(".sav"))
                .ok_or_else(|| HandlerError::Other(format!("unknown export: {name}")))?;
            let (stem, want_dps) = match stem.strip_suffix("_dps") {
                Some(stem) => (stem, true),
                None => (stem, false),
            };
            let (_, loaded) = session
                .loaded_players
                .iter()
                .find(|(uid, _)| download_player_stem(uid) == stem)
                .ok_or_else(|| HandlerError::Other(format!("unknown export: {name}")))?;
            let save = if want_dps {
                loaded.dps.as_ref().ok_or_else(|| missing(name))?
            } else {
                &loaded.sav
            };
            Ok(psp_core::savio::write_gvas_bytes(save)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestContext;

    const UID: &str = "084df277-0000-0000-0000-000000000000";

    fn world1_level_gvas_bytes() -> Vec<u8> {
        let sav = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/saves/world1/Level.sav"
        ))
        .expect("read world1");
        let save = psp_core::savio::read_sav_bytes(&sav).expect("parse");
        psp_core::savio::write_gvas_bytes(&save).expect("gvas")
    }

    /// The whole point of the bytes path: a save goes in and comes back out
    /// without ever being base64'd or serialized into a JSON frame.
    #[tokio::test]
    async fn staged_bytes_load_and_export_round_trip_without_base64() {
        let mut test = TestContext::new(|_| {}).await;
        let gvas = world1_level_gvas_bytes();

        let mut staged = StagedGvas::default();
        staged.stage("level", "", gvas.clone()).expect("stage level");
        let payload = staged.take("world1".to_string()).expect("payload");

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
            handle_load_save_gvas_bytes(payload, &mut ctx).await.expect("load");
        }
        assert_eq!(next_non_progress_frame(&mut test)["type"], "loaded_save_files");

        let session = test.session.save.as_ref().expect("a save is loaded");
        let manifest = export_manifest(session);
        assert_eq!(manifest.names, vec!["Level.sav".to_string()]);
        assert_eq!(export_file(session, "Level.sav").expect("export"), gvas);
    }

    #[tokio::test]
    async fn export_manifest_lists_level_meta_when_one_is_loaded() {
        let mut test = TestContext::new(|_| {}).await;
        let level = world1_level_gvas_bytes();
        let meta_sav = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/saves/world1/LevelMeta.sav"
        ))
        .expect("read LevelMeta");
        let meta = psp_core::savio::write_gvas_bytes(
            &psp_core::savio::read_sav_bytes(&meta_sav).expect("parse meta"),
        )
        .expect("meta gvas");

        let mut staged = StagedGvas::default();
        staged.stage("level", "", level).expect("stage level");
        staged.stage("level_meta", "", meta).expect("stage meta");
        let payload = staged.take("world1".to_string()).expect("payload");

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
            handle_load_save_gvas_bytes(payload, &mut ctx).await.expect("load");
        }

        let session = test.session.save.as_ref().expect("a save is loaded");
        let manifest = export_manifest(session);
        assert!(
            manifest.names.contains(&"LevelMeta.sav".to_string()),
            "got {:?}",
            manifest.names
        );
        assert_eq!(&export_file(session, "LevelMeta.sav").expect("export")[0..4], b"GVAS");
    }

    #[tokio::test]
    async fn exporting_an_unknown_name_is_rejected() {
        let mut test = TestContext::new(|_| {}).await;
        let mut staged = StagedGvas::default();
        staged.stage("level", "", world1_level_gvas_bytes()).expect("stage");
        let payload = staged.take("world1".to_string()).expect("payload");

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
            handle_load_save_gvas_bytes(payload, &mut ctx).await.expect("load");
        }

        let session = test.session.save.as_ref().expect("a save is loaded");
        let error = export_file(session, "Nope.sav").expect_err("unknown name");
        assert!(error.to_string().contains("Nope.sav"), "got {error}");
    }

    #[test]
    fn staging_pairs_a_players_sav_and_dps_under_one_uid() {
        let mut staged = StagedGvas::default();
        staged.stage("level", "", vec![1, 2, 3]).expect("level");
        staged.stage("player_dps", UID, vec![4]).expect("dps");
        staged.stage("player_sav", UID, vec![5]).expect("sav");

        let payload = staged.take("world1".to_string()).expect("payload");

        assert_eq!(payload.save_id, "world1");
        assert_eq!(payload.level, vec![1, 2, 3]);
        assert_eq!(payload.players.len(), 1, "one player, not one per file");
        assert_eq!(payload.players[0].uid, UID.parse::<Uuid>().unwrap());
        assert_eq!(payload.players[0].sav, vec![5]);
        assert_eq!(payload.players[0].dps.as_deref(), Some(&[4u8][..]));
    }

    #[test]
    fn staging_rejects_an_unknown_slot() {
        let mut staged = StagedGvas::default();
        let error = staged.stage("bogus", "", vec![1]).expect_err("unknown slot");
        assert!(error.to_string().contains("bogus"), "got {error}");
    }

    #[test]
    fn staging_without_a_level_is_rejected() {
        let mut staged = StagedGvas::default();
        staged.stage("player_sav", UID, vec![5]).expect("sav");
        let error = staged.take("world1".to_string()).expect_err("no level");
        assert!(error.to_string().contains("Level"), "got {error}");
    }

    /// On wasm32 the staged buffers are the largest allocation in the process,
    /// so a second load must not be able to resurrect the previous save's.
    #[test]
    fn taking_the_payload_empties_the_staging_area() {
        let mut staged = StagedGvas::default();
        staged.stage("level", "", vec![1, 2, 3]).expect("level");
        staged.stage("player_sav", UID, vec![5]).expect("sav");
        staged.take("world1".to_string()).expect("payload");

        let error = staged.take("world1".to_string()).expect_err("drained");
        assert!(error.to_string().contains("Level"), "got {error}");
    }

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

}
