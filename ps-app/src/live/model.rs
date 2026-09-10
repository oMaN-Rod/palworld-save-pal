use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveActor {
    pub id: String,
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yaw: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_hp: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub species: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveFrame {
    pub seq: u64,
    pub source: LiveSourceKind,
    pub captured_at_ms: u64,
    pub observed_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingame_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingame_days: Option<i64>,
    pub actors: Vec<LiveActor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LiveSourceKind {
    File,
    Rest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceHealth {
    Idle,
    Waiting,
    Auth,
    Down,
    Stale,
    Ok,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalSourceStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<LiveSourceKind>,
    pub health: SourceHealth,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_frame_ms: Option<u64>,
    pub actor_count: usize,
}

impl Default for SignalSourceStatus {
    fn default() -> Self {
        Self { kind: None, health: SourceHealth::Idle, error: None, last_frame_ms: None, actor_count: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_serializes_camel_case_and_omits_empty_options() {
        let f = LiveFrame {
            seq: 7,
            source: LiveSourceKind::File,
            captured_at_ms: 1_756_500_000_000,
            observed_at_ms: 1_756_500_002_000,
            fps: None,
            ingame_time: Some("07:09".into()),
            ingame_days: Some(30),
            actors: vec![LiveActor {
                id: "abc".into(), kind: "player".into(),
                x: -201.1, y: -138238.3, z: 2916.1,
                yaw: Some(26.3), name: Some("O".into()), level: Some(80),
                hp: Some(8575), max_hp: Some(8575),
                guild: Some("DeBugging".into()), owner: None,
                species: None, active: Some(true),
            }],
        };
        let v: serde_json::Value = serde_json::to_value(&f).unwrap();
        assert_eq!(v["source"], "file");
        assert_eq!(v["capturedAtMs"], 1_756_500_000_000u64);
        assert_eq!(v["actors"][0]["maxHp"], 8575);
        assert!(v.get("fps").is_none());
        assert!(v["actors"][0].get("owner").is_none());
        let back: LiveFrame = serde_json::from_value(v).unwrap();
        assert_eq!(back, f);
    }
}
