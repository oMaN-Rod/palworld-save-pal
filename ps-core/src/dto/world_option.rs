use serde::{Deserialize, Serialize};

/// Response to `get_world_option`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldOptionDto {
    /// False when the save carries no WorldOption.sav, or it failed to parse.
    pub present: bool,
    pub version: i32,
    /// PRESENT settings only, in GVAS order. The UI supplies defaults for the rest.
    pub settings: Vec<WorldOptionEntryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldOptionEntryDto {
    pub key: String,
    /// Lowercase kind tag: bool|int|float|str|name|enum|enum_array|name_array.
    pub kind: String,
    pub value: serde_json::Value,
}

/// `update_world_option` payload: ONLY the changed keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldOptionPatchDto {
    pub entries: Vec<WorldOptionPatchEntryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldOptionPatchEntryDto {
    pub key: String,
    pub value: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_option_dto_serializes_expected_shape() {
        let dto = WorldOptionDto {
            present: true,
            version: 101,
            settings: vec![WorldOptionEntryDto {
                key: "ExpRate".into(),
                kind: "float".into(),
                value: serde_json::json!(20.0),
            }],
        };
        let text = serde_json::to_string(&dto).unwrap();
        assert_eq!(
            text,
            r#"{"present":true,"version":101,"settings":[{"key":"ExpRate","kind":"float","value":20.0}]}"#
        );
    }

    #[test]
    fn absent_world_option_serializes_with_empty_settings() {
        let dto = WorldOptionDto { present: false, version: 0, settings: vec![] };
        let text = serde_json::to_string(&dto).unwrap();
        assert_eq!(text, r#"{"present":false,"version":0,"settings":[]}"#);
    }

    #[test]
    fn patch_dto_deserializes_from_frontend_shape() {
        let payload = r#"{"entries":[{"key":"ExpRate","value":5.0},
                                     {"key":"CrossplayPlatforms","value":["EPalAllowConnectPlatform::Steam"]}]}"#;
        let patch: WorldOptionPatchDto = serde_json::from_str(payload).unwrap();
        assert_eq!(patch.entries.len(), 2);
        assert_eq!(patch.entries[0].key, "ExpRate");
        assert_eq!(patch.entries[1].value, serde_json::json!(["EPalAllowConnectPlatform::Steam"]));
    }
}
