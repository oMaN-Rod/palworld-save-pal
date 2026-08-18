//! Dev-only verification harness: loads a real save directory, computes the
//! Overview stats, and dumps them as JSON to stdout (or a path given as the
//! first argument). Used to diff PSP's Rust overview against PalSavTools'
//! reference implementation over the same save.
//!
//! Usage: cargo run -p psp-core --example overview_dump -- <save_dir> [out.json]

use std::collections::BTreeMap;
use std::path::PathBuf;

use psp_core::domain::overview::overview_stats;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};

fn main() {
    let save_dir = std::env::args()
        .nth(1)
        .expect("usage: overview_dump <save_dir> [out.json]");
    let out_path = std::env::args().nth(2);
    let data_dir = std::env::var("PSP_DATA_DIR").unwrap_or_else(|_| "data/json".to_string());

    let level = std::fs::read(format!("{save_dir}/Level.sav")).expect("read Level.sav");
    let level_meta = std::fs::read(format!("{save_dir}/LevelMeta.sav")).ok();

    let mut player_file_refs = BTreeMap::new();
    let players_dir = PathBuf::from(&save_dir).join("Players");
    if players_dir.is_dir() {
        for entry in std::fs::read_dir(&players_dir).expect("read Players dir") {
            let path = entry.expect("dir entry").path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let Some(uid_hex) = stem.strip_suffix("_dps") else {
                continue;
            };
            let uid = decode_player_uid(uid_hex).expect("parse player uid");
            let dps = format!("{uid_hex}_dps.sav");
            let dps_path = players_dir.join(&dps);
            let entry = if dps_path.exists() {
                PlayerFileData::Paths {
                    sav: Some(path),
                    dps: Some(dps_path),
                }
            } else {
                PlayerFileData::Paths {
                    sav: Some(path),
                    dps: None,
                }
            };
            player_file_refs.entry(uid).or_insert(entry);
        }
    }

    let game_data = GameData::load(std::path::Path::new(&data_dir)).expect("load game data");
    let session = SaveSession::load(
        SaveKind::InMemory,
        "verification".to_string(),
        "steam",
        &level,
        level_meta.as_deref(),
        None,
        player_file_refs,
        None,
        false,
        &null_progress(),
    )
    .expect("load save session");

    let stats = overview_stats(&session, &game_data).expect("compute overview stats");
    let json = serde_json::to_string_pretty(&stats).expect("serialize stats");
    match out_path {
        Some(path) => std::fs::write(path, json).expect("write output"),
        None => println!("{json}"),
    }
}

/// Player save files are named `<HEXUID>.sav` where the hex is the canonical
/// UUID string without dashes, uppercase.
fn decode_player_uid(hex: &str) -> Option<uuid::Uuid> {
    let dashed = if hex.len() == 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    } else {
        hex.to_string()
    };
    uuid::Uuid::parse_str(&dashed).ok()
}
