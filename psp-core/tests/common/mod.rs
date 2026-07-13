use psp_core::session::{PlayerFileData, SaveKind, SaveSession};
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Loads the private corpus save named by `PSP_TEST_SAVE_DIR`, or `None`
/// (skipping) when that env var is unset.
pub fn load_corpus_session() -> Option<SaveSession> {
    let Ok(save_dir) = std::env::var("PSP_TEST_SAVE_DIR") else {
        eprintln!("PSP_TEST_SAVE_DIR not set; skipping corpus test");
        return None;
    };
    let save_dir = PathBuf::from(save_dir);
    let level_sav_bytes = std::fs::read(save_dir.join("Level.sav")).expect("read Level.sav");
    let level_meta_bytes = std::fs::read(save_dir.join("LevelMeta.sav")).ok();

    let mut player_file_refs: BTreeMap<Uuid, PlayerFileData> = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(save_dir.join("Players")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "sav") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let (uid_part, is_dps) = match stem.strip_suffix("_dps") {
                Some(base) => (base, true),
                None => (stem, false),
            };
            let Ok(uid) = uid_part.parse::<Uuid>() else {
                continue;
            };
            let file_ref = player_file_refs
                .entry(uid)
                .or_insert(PlayerFileData::Paths {
                    sav: None,
                    dps: None,
                });
            if let PlayerFileData::Paths { sav, dps } = file_ref {
                if is_dps {
                    *dps = Some(path);
                } else {
                    *sav = Some(path);
                }
            }
        }
    }

    let session = SaveSession::load(
        SaveKind::Steam {
            level_path: save_dir.join("Level.sav"),
        },
        save_dir.to_string_lossy().into_owned(),
        "steam",
        &level_sav_bytes,
        level_meta_bytes.as_deref(),
        player_file_refs,
        None,
        true,
        &psp_core::progress::null_progress(),
    )
    .expect("load corpus session");

    Some(session)
}

/// Loads a committed fixture save from `tests/fixtures/saves/<name>/`. Never
/// env-gated, so tests built on it always run; panics on failure, since a
/// missing or broken checked-in fixture is a repo problem, not a skip.
pub fn load_fixture_session(name: &str) -> SaveSession {
    let save_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/saves")
        .join(name);
    let level_sav_bytes =
        std::fs::read(save_dir.join("Level.sav")).expect("read fixture Level.sav");
    let level_meta_bytes = std::fs::read(save_dir.join("LevelMeta.sav")).ok();

    let mut player_file_refs: BTreeMap<Uuid, PlayerFileData> = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(save_dir.join("Players")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "sav") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let (uid_part, is_dps) = match stem.strip_suffix("_dps") {
                Some(base) => (base, true),
                None => (stem, false),
            };
            let Ok(uid) = uid_part.parse::<Uuid>() else {
                continue;
            };
            let file_ref = player_file_refs
                .entry(uid)
                .or_insert(PlayerFileData::Paths {
                    sav: None,
                    dps: None,
                });
            if let PlayerFileData::Paths { sav, dps } = file_ref {
                if is_dps {
                    *dps = Some(path);
                } else {
                    *sav = Some(path);
                }
            }
        }
    }

    SaveSession::load(
        SaveKind::Steam {
            level_path: save_dir.join("Level.sav"),
        },
        save_dir.to_string_lossy().into_owned(),
        "steam",
        &level_sav_bytes,
        level_meta_bytes.as_deref(),
        player_file_refs,
        None,
        true,
        &psp_core::progress::null_progress(),
    )
    .expect("load fixture session")
}
