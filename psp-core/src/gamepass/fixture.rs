//! Synthetic wgs container trees for tests. Real gamepass saves are machine-specific,
//! so tests package ordinary .sav bytes into the container format themselves.

use std::path::{Path, PathBuf};

use crate::error::CoreError;
use crate::gamepass::format::{ContainerIndex, Filetime};
use crate::gamepass::store::create_container;

pub struct SyntheticPlayer {
    pub id: uuid::Uuid,
    pub sav: Vec<u8>,
    pub dps: Option<Vec<u8>>,
}

pub struct SyntheticSave {
    pub save_id: String,
    pub level_sav: Vec<u8>,
    pub level_meta: Option<Vec<u8>>,
    pub local_data: Option<Vec<u8>>,
    pub world_option: Option<Vec<u8>>,
    pub players: Vec<SyntheticPlayer>,
}

pub fn build_wgs_tree(root: &Path, saves: &[SyntheticSave]) -> Result<PathBuf, CoreError> {
    let container_dir = root
        .join("SystemAppData")
        .join("wgs")
        .join("0009000000000000_00000000000000000000000000000000");
    std::fs::create_dir_all(&container_dir)?;

    let mut index = ContainerIndex {
        flag1: 0,
        package_name: "PocketpairInc.Palworld_ad4psfrxyesvt".to_string(),
        mtime: Filetime::now(),
        flag2: 0,
        index_uuid: String::new(),
        unknown: 0,
        containers: Vec::new(),
    };

    for save in saves {
        index.containers.push(create_container(
            &container_dir,
            &save.save_id,
            &save.level_sav,
            "Data",
            "Level",
        )?);
        if let Some(meta) = &save.level_meta {
            index.containers.push(create_container(
                &container_dir,
                &save.save_id,
                meta,
                "Data",
                "LevelMeta",
            )?);
        }
        if let Some(local_data) = &save.local_data {
            index.containers.push(create_container(
                &container_dir,
                &save.save_id,
                local_data,
                "Data",
                "LocalData",
            )?);
        }
        if let Some(world_option) = &save.world_option {
            index.containers.push(create_container(
                &container_dir,
                &save.save_id,
                world_option,
                "Data",
                "WorldOption",
            )?);
        }
        for player in &save.players {
            let player_hex = player.id.as_simple().to_string().to_uppercase();
            index.containers.push(create_container(
                &container_dir,
                &save.save_id,
                &player.sav,
                "Data",
                &format!("Players-{player_hex}"),
            )?);
            if let Some(dps) = &player.dps {
                index.containers.push(create_container(
                    &container_dir,
                    &save.save_id,
                    dps,
                    "Data",
                    &format!("Players-{player_hex}_dps"),
                )?);
            }
        }
    }

    index.write_to_dir(&container_dir)?;
    Ok(container_dir)
}

/// Committed directory of real PlZ/zlib reference saves (`Level.sav`,
/// `LevelMeta.sav`, `LocalData.sav`, `00000000000000000000000000000001.sav`),
/// copied from the upstream palworld-save-tools public test corpus. Replaces the
/// former external sibling-checkout dependency, so the tests that read these
/// bytes run unconditionally on a clean checkout instead of silently skipping.
pub fn reference_saves_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/reference_saves")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepass::format::ContainerIndex;
    use crate::gamepass::store::read_first_blob;

    #[test]
    fn build_wgs_tree_produces_readable_container_dir() {
        let temp = tempfile::tempdir().unwrap();
        let player_id = uuid::Uuid::new_v4();
        let save = SyntheticSave {
            save_id: "0123456789ABCDEF0123456789ABCDEF".to_string(),
            level_sav: b"LEVEL".to_vec(),
            level_meta: Some(b"META".to_vec()),
            local_data: None,
            world_option: None,
            players: vec![SyntheticPlayer {
                id: player_id,
                sav: b"PLAYER".to_vec(),
                dps: Some(b"DPS".to_vec()),
            }],
        };
        let container_dir = build_wgs_tree(temp.path(), &[save]).unwrap();
        assert!(container_dir.join("containers.index").exists());
        assert!(crate::gamepass::store::is_wgs_container_dir_name(
            &container_dir.file_name().unwrap().to_string_lossy()
        ));

        let index = ContainerIndex::read_from_dir(&container_dir).unwrap();
        assert_eq!(index.containers.len(), 4); // Level, LevelMeta, player sav, player dps
        let latest = index.latest_save_containers("0123456789ABCDEF0123456789ABCDEF");
        let level = latest.get("Level").unwrap();
        let (seq, blob) = read_first_blob(&container_dir, level).unwrap().unwrap();
        assert_eq!((seq, blob.as_slice()), (1, b"LEVEL".as_slice()));

        let player_hex = player_id.as_simple().to_string().to_uppercase();
        assert!(latest.get(&format!("Players-{player_hex}")).is_some());
        assert!(latest.get(&format!("Players-{player_hex}_dps")).is_some());
    }
}
