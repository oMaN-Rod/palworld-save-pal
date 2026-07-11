use psp_core::session::{PlayerFileData, SaveKind, SaveSession};
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Loads the corpus save named by `PSP_TEST_SAVE_DIR` into a `SaveSession`
/// the same way the real load path (`handle_select_save`) does — via
/// `SaveSession::load`, discovering `Players/*.sav` file references first.
/// Returns `None` (after printing a skip notice) when the env var is unset,
/// matching every other corpus-gated test in this workspace
/// (`session.rs`'s `test_load_real_steam_save`,
/// `psp-server/tests/load_path.rs`).
///
/// Deviation from the brief: the brief's version of this helper called
/// `SaveSession::new_for_tests` plus `psp_core::savio::read_sav_bytes` and
/// `psp_core::domain::summaries::extract_player_summaries` /
/// `extract_guild_summaries` — none of which exist. `savio` is Task 12
/// scope (not yet built), and summary extraction is one combined
/// `extract_summaries` function, not two. `SaveSession::load` already does
/// everything this helper needs (parse Level.sav/LevelMeta.sav, build the
/// Phase-1 indexes, extract both summary maps) through a single already-`pub`
/// entry point, so this helper builds `player_file_refs` itself (mirroring
/// `psp-server`'s own `collect_player_file_refs`, which isn't reachable from
/// here — it's private to the `psp-server` crate) and calls straight into
/// `SaveSession::load` instead.
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

/// Loads a fixture save checked into the repo at
/// `tests/fixtures/saves/<name>/` -- unlike `load_corpus_session`, NOT gated
/// by `PSP_TEST_SAVE_DIR`, since these fixtures are committed (`git log --
/// tests/fixtures/saves/world1/Level.sav` has history) and always present, so
/// a test built on this helper always runs rather than silently skipping.
/// Panics loudly on any load failure: a missing/broken checked-in fixture is
/// a repo problem, not a "skip and move on" condition. Duplicates
/// `load_corpus_session`'s small `Players/` directory walk rather than
/// factoring it out, so that function's already-verified behavior stays
/// untouched.
pub fn load_fixture_session(name: &str) -> SaveSession {
    let save_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/saves")
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
