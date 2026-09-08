#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BridgeEnvelope {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeErrorData {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEndpointFile {
    pub protocol_version: i64,
    pub port: u16,
    pub token: String,
    pub pid: u32,
    pub started_at: String,
}

pub const BRIDGE_PROTOCOL_VERSION: i64 = 1;
