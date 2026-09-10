use std::sync::OnceLock;

use serde::Serialize;

use super::fields::Access;
use super::{gamedata, raw, save_read, save_write, services};
use crate::manifest::Capability;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ApiType {
    Nil,
    Boolean,
    Integer,
    Number,
    String,
    Table,
    Handle(&'static str),
    Iterator(&'static str),
    Union(&'static [ApiType]),
    List(&'static ApiType),
    Map { key: &'static ApiType, value: &'static ApiType },
    /// Two or more values returned side by side, in the order Lua receives
    /// them. Distinct from `Union`, which is one value of several possible
    /// types.
    Multi(&'static [ApiType]),
    Any,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApiParam {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub ty: ApiType,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ApiFunction {
    pub name: &'static str,
    pub params: &'static [ApiParam],
    pub returns: ApiType,
    pub doc: &'static str,
    /// `None` means it inherits the gate of the global or handle it lives on.
    pub capability: Option<Capability>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApiField {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub ty: ApiType,
    pub access: Access,
    pub doc: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApiGlobal {
    pub name: &'static str,
    /// `None` for a global installed regardless of granted capabilities.
    pub capability: Option<Capability>,
    pub functions: &'static [ApiFunction],
    pub fields: &'static [ApiField],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApiHandle {
    pub name: &'static str,
    pub fields: &'static [ApiField],
    pub methods: &'static [ApiFunction],
    pub capability: Option<Capability>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApiDefinition {
    pub globals: Vec<ApiGlobal>,
    pub handles: Vec<ApiHandle>,
}

/// Not a `const`: concatenating two `&'static [ApiFunction]` has no const form.
static SAVE_FUNCTIONS: OnceLock<Vec<ApiFunction>> = OnceLock::new();

fn save_functions() -> &'static [ApiFunction] {
    SAVE_FUNCTIONS
        .get_or_init(|| [save_read::SAVE_READ_FUNCTIONS, save_write::SAVE_WRITE_FUNCTIONS].concat())
        .as_slice()
}

pub fn api_definition() -> ApiDefinition {
    ApiDefinition {
        globals: vec![
            ApiGlobal {
                name: "raw",
                capability: Some(Capability::SaveRaw),
                functions: raw::RAW_FUNCTIONS,
                fields: &[],
            },
            ApiGlobal {
                name: "save",
                capability: Some(Capability::SaveRead),
                functions: save_functions(),
                fields: &[],
            },
            ApiGlobal {
                name: "gamedata",
                capability: Some(Capability::GameData),
                functions: gamedata::GAMEDATA_FUNCTIONS,
                fields: &[],
            },
            ApiGlobal {
                name: "progress",
                capability: None,
                functions: services::PROGRESS_FUNCTIONS,
                fields: &[],
            },
            ApiGlobal {
                name: "ctx",
                capability: None,
                functions: &[],
                fields: services::CTX_FIELDS,
            },
            ApiGlobal {
                name: "log",
                capability: Some(Capability::Log),
                functions: services::LOG_FUNCTIONS,
                fields: &[],
            },
            ApiGlobal {
                name: "storage",
                capability: Some(Capability::Storage),
                functions: services::STORAGE_FUNCTIONS,
                fields: &[],
            },
            ApiGlobal {
                name: "ui",
                capability: Some(Capability::UiDialog),
                functions: services::UI_FUNCTIONS,
                fields: &[],
            },
        ],
        handles: save_read::handle_types().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_definition_describes_gamedata_exactly() {
        let def = api_definition();
        let gamedata = def
            .globals
            .iter()
            .find(|g| g.name == "gamedata")
            .expect("gamedata is described");

        assert_eq!(gamedata.capability, Some(Capability::GameData));

        let mut names: Vec<&str> = gamedata.functions.iter().map(|f| f.name).collect();
        names.sort_unstable();
        assert_eq!(names, ["catalogs", "get", "is_valid_item", "is_valid_pal", "keys", "version"]);

        for function in gamedata.functions {
            assert!(
                !function.doc.is_empty(),
                "{} has no doc string; hover would show nothing",
                function.name
            );
        }
    }

    #[test]
    fn the_save_table_records_its_two_capabilities_separately() {
        let def = api_definition();
        let save = def
            .globals
            .iter()
            .find(|g| g.name == "save")
            .expect("save is described");

        assert_eq!(save.capability, Some(Capability::SaveRead));

        let write_half: Vec<&str> = save
            .functions
            .iter()
            .filter(|f| f.capability == Some(Capability::SaveWrite))
            .map(|f| f.name)
            .collect();
        assert!(
            write_half.contains(&"unlock_private_chests"),
            "the write half must be marked: {write_half:?}"
        );

        let read_half: Vec<&str> = save
            .functions
            .iter()
            .filter(|f| f.capability.is_none())
            .map(|f| f.name)
            .collect();
        assert!(
            read_half.contains(&"players"),
            "the read half inherits the table's gate: {read_half:?}"
        );
    }

    #[test]
    fn every_global_records_its_capability_or_is_deliberately_ungated() {
        const UNGATED: &[&str] = &["progress", "ctx"];
        for global in &api_definition().globals {
            if UNGATED.contains(&global.name) {
                assert_eq!(global.capability, None, "{} is ungated", global.name);
            } else {
                assert!(global.capability.is_some(), "{} has no gate", global.name);
            }
        }
    }

    #[test]
    fn the_guild_handle_describes_the_fields_its_resolver_answers() {
        let def = api_definition();
        let guild = def
            .handles
            .iter()
            .find(|h| h.name == "guild")
            .expect("the guild handle is described");

        let mut names: Vec<&str> = guild.fields.iter().map(|f| f.name).collect();
        names.sort_unstable();
        assert!(
            names.contains(&"chest_container_id"),
            "the chest id field must be described: {names:?}"
        );

        let chest = guild
            .fields
            .iter()
            .find(|f| f.name == "chest_container_id")
            .expect("described");
        assert_eq!(
            chest.ty,
            ApiType::Union(&[ApiType::String, ApiType::Nil]),
            "a guild with no chest yields nil, and the type must say so"
        );
    }

    #[test]
    fn every_handle_type_describes_its_fields() {
        for handle in &api_definition().handles {
            assert!(
                !handle.fields.is_empty() || !handle.methods.is_empty(),
                "{} describes nothing",
                handle.name
            );
        }
    }

    #[test]
    fn the_save_functions_merge_is_built_at_most_once() {
        let first = api_definition().globals.into_iter().find(|g| g.name == "save").unwrap().functions;
        let second = api_definition().globals.into_iter().find(|g| g.name == "save").unwrap().functions;
        assert_eq!(
            first.as_ptr(),
            second.as_ptr(),
            "save's merged functions must be the same cached allocation across calls, not rebuilt each time"
        );
    }

    #[test]
    fn every_iterator_return_names_a_described_handle() {
        fn iterator_target(ty: &ApiType) -> Option<&'static str> {
            match ty {
                ApiType::Iterator(name) => Some(*name),
                _ => None,
            }
        }

        let def = api_definition();
        let handle_names: Vec<&str> = def.handles.iter().map(|h| h.name).collect();

        let mut owned_functions: Vec<(&str, &ApiFunction)> = Vec::new();
        for global in &def.globals {
            for function in global.functions {
                owned_functions.push((global.name, function));
            }
        }
        for handle in &def.handles {
            for method in handle.methods {
                owned_functions.push((handle.name, method));
            }
        }

        for (owner, function) in owned_functions {
            let Some(target) = iterator_target(&function.returns) else {
                continue;
            };
            assert!(
                handle_names.contains(&target),
                "{owner}.{} returns Iterator(\"{target}\"), but no handle named \"{target}\" is \
                 described (described handles: {handle_names:?})",
                function.name
            );
        }
    }
}
