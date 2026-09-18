use serde::{Deserialize, Serialize};

use super::types::RouteKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModInfoRoute {
    pub path: String,
    pub kind: RouteKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModInfoJson {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub routes: Vec<ModInfoRoute>,
}

pub fn parse_modinfo(json: &str) -> Result<ModInfoJson, serde_json::Error> {
    serde_json::from_str(json)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRule {
    #[serde(rename = "Type", default)]
    pub rule_type: String,
    #[serde(rename = "IsServer", default)]
    pub is_server: bool,
    #[serde(rename = "Targets", default)]
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopInfo {
    #[serde(rename = "PackageName", default)]
    pub package_name: String,
    #[serde(rename = "ModName", default)]
    pub mod_name: String,
    #[serde(rename = "Version", default)]
    pub version: String,
    #[serde(rename = "Author", default)]
    pub author: String,
    #[serde(rename = "Dependencies", default)]
    pub dependencies: Vec<String>,
    #[serde(rename = "InstallRule", default)]
    pub install_rules: Vec<InstallRule>,
}

pub fn parse_workshop_info(json: &str) -> Result<WorkshopInfo, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn route_kind_for_rule(rule_type: &str) -> RouteKind {
    match rule_type.to_ascii_lowercase().as_str() {
        "ue4ss" | "lua" => RouteKind::Ue4ss,
        "logicmods" => RouteKind::LogicMods,
        "paks" | "pak" => RouteKind::Pak,
        "palschema" => RouteKind::PalSchema,
        _ => RouteKind::Passthrough,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::RouteKind;

    #[test]
    fn parses_modinfo_routes() {
        let json = r#"{"name":"Cool","version":"2.1","routes":[{"path":"a/b.pak","kind":"logicmods"},{"path":"x.lua","kind":"ue4ss"}]}"#;
        let m = parse_modinfo(json).unwrap();
        assert_eq!(m.name.as_deref(), Some("Cool"));
        assert_eq!(m.routes.len(), 2);
        assert_eq!(m.routes[0].kind, RouteKind::LogicMods);
    }

    #[test]
    fn modinfo_without_routes_is_valid() {
        let m = parse_modinfo(r#"{"name":"Cool"}"#).unwrap();
        assert!(m.routes.is_empty());
    }

    #[test]
    fn parses_workshop_info_with_game_field_names() {
        let json = r#"{
          "PackageName":"CoolPkg","ModName":"Cool Mod","Version":"1.2","Author":"me",
          "Dependencies":["UE4SS"],
          "InstallRule":[{"Type":"UE4SS","IsServer":false,"Targets":["Mods/Cool"]},{"Type":"Paks","IsServer":true,"Targets":["Cool_P.pak"]}]
        }"#;
        let w = parse_workshop_info(json).unwrap();
        assert_eq!(w.package_name, "CoolPkg");
        assert_eq!(w.dependencies, vec!["UE4SS".to_string()]);
        assert_eq!(w.install_rules.len(), 2);
        assert!(w.install_rules[1].is_server);
        assert_eq!(w.install_rules[0].targets, vec!["Mods/Cool".to_string()]);
    }

    #[test]
    fn missing_optional_fields_default() {
        let w = parse_workshop_info(r#"{"PackageName":"P"}"#).unwrap();
        assert_eq!(w.mod_name, "");
        assert!(w.install_rules.is_empty());
    }

    #[test]
    fn rule_type_mapping() {
        assert_eq!(route_kind_for_rule("UE4SS"), RouteKind::Ue4ss);
        assert_eq!(route_kind_for_rule("lua"), RouteKind::Ue4ss);
        assert_eq!(route_kind_for_rule("LogicMods"), RouteKind::LogicMods);
        assert_eq!(route_kind_for_rule("Paks"), RouteKind::Pak);
        assert_eq!(route_kind_for_rule("PalSchema"), RouteKind::PalSchema);
        assert_eq!(route_kind_for_rule("Other"), RouteKind::Passthrough);
    }
}
