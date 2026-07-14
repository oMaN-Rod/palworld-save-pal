//! Wire DTOs for gamepass save scanning.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GamepassContainerInfo {
    /// Container name suffix after the save id, e.g. "Level", "Players-<HEX>".
    pub container_type: String,
    pub seq: u8,
    /// Unix timestamp (float) of the container mtime.
    pub last_modified: f64,
    pub size: u64,
    pub container_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GamepassSaveData {
    pub save_id: String,
    pub world_name: String,
    pub player_count: usize,
    pub last_modified: f64,
    pub total_size: u64,
    /// ALL container versions for this save (not just the latest), sorted by
    /// (container_type asc, seq desc).
    pub containers: Vec<GamepassContainerInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamepass_save_data_serializes_with_python_field_names_and_order() {
        let save = GamepassSaveData {
            save_id: "8C4B8D0846A067700F2E54BBDA266D0E".to_string(),
            world_name: "My World".to_string(),
            player_count: 2,
            last_modified: 1720000000.5,
            total_size: 4096,
            containers: vec![GamepassContainerInfo {
                container_type: "Level".to_string(),
                seq: 1,
                last_modified: 1720000000.5,
                size: 2048,
                container_name: "8C4B8D0846A067700F2E54BBDA266D0E-Level".to_string(),
            }],
        };
        let json = serde_json::to_string(&save).unwrap();
        assert_eq!(
            json,
            r#"{"save_id":"8C4B8D0846A067700F2E54BBDA266D0E","world_name":"My World","player_count":2,"last_modified":1720000000.5,"total_size":4096,"containers":[{"container_type":"Level","seq":1,"last_modified":1720000000.5,"size":2048,"container_name":"8C4B8D0846A067700F2E54BBDA266D0E-Level"}]}"#
        );
    }
}
