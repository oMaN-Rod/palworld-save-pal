/// The wire envelope: {"type": <string>, "data": <any>}.
/// `data` defaults to JSON null when absent — the frontend omits it for
/// payload-less requests (JSON.stringify drops undefined).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Envelope {
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::Envelope;

    #[test]
    fn data_defaults_to_null_when_absent() {
        let envelope: Envelope = serde_json::from_str(r#"{"type":"get_version"}"#).unwrap();
        assert_eq!(envelope.message_type, "get_version");
        assert!(envelope.data.is_null());
    }

    #[test]
    fn missing_type_is_a_parse_error() {
        assert!(serde_json::from_str::<Envelope>(r#"{"data":1}"#).is_err());
    }

    #[test]
    fn round_trips_with_payload() {
        let envelope: Envelope =
            serde_json::from_str(r#"{"type":"update_settings","data":{"language":"en"}}"#).unwrap();
        assert_eq!(envelope.data["language"], "en");
        let text = serde_json::to_string(&envelope).unwrap();
        assert_eq!(
            text,
            r#"{"type":"update_settings","data":{"language":"en"}}"#
        );
    }
}
