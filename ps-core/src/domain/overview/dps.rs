//! The legality scan over each player's Dimensional Pal Storage, which lives
//! in per-player `_dps.sav` files the character map never sees.
//!
//! A `_dps.sav` decompresses to ~70 MB of mostly-empty slots and takes about a
//! second to parse, so unloaded players are only parsed on request
//! ([`fill_cache`]) and the result is kept in [`SaveSession::dps_scans`] for the
//! rest of the session. Their files cannot change underneath the session, and
//! every path that rewires a player's file reference drops that player's
//! entry. Loaded players are scanned from their in-memory tree on every call,
//! so edits show up immediately.

use crate::dto::overview::OverviewAnomalyRow;
use crate::gamedata::GameData;
use crate::progress::ProgressSink;
use crate::props;
use crate::session::{parse_palworld_save, PlayerFileData, SaveSession};
use crate::ue::{Properties, PropertyKey, Save, StructValue};

use super::anomalies::{flagged_row, SOURCE_DPS};
use super::catalogs::OverviewCatalogs;
use super::classify::canonical_character_key;
use super::current_level;
use super::illegal_pals::detect_pal_issues;

#[derive(Debug, Default)]
pub(crate) struct DpsScan {
    pub(crate) pal_count: i64,
    pub(crate) flagged: Vec<OverviewAnomalyRow>,
}

pub(crate) fn is_pending(
    session: &SaveSession,
    uid: &uuid::Uuid,
    file_ref: &PlayerFileData,
) -> bool {
    let has_dps = match file_ref {
        PlayerFileData::Paths { dps, .. } => dps.is_some(),
        PlayerFileData::Bytes { dps, .. } => dps.is_some(),
    };
    has_dps && !session.loaded_players.contains_key(uid) && !session.dps_scans.contains_key(uid)
}

/// A file that fails to parse caches as empty so it is not re-read on every
/// scan; a read error is left uncached and retried.
pub(crate) fn fill_cache(
    session: &mut SaveSession,
    catalogs: &OverviewCatalogs,
    game_data: &GameData,
    progress: &ProgressSink,
) {
    let pending: Vec<uuid::Uuid> = session
        .player_file_refs
        .iter()
        .filter(|(uid, file_ref)| is_pending(session, uid, file_ref))
        .map(|(uid, _)| *uid)
        .collect();
    for (index, uid) in pending.iter().enumerate() {
        let name = session.player_summaries.get(uid).map_or_else(
            || uid.to_string()[..8].to_string(),
            |summary| summary.nickname.clone(),
        );
        progress(&format!(
            "Scanning Dimensional Pal Storage {}/{}: {name}",
            index + 1,
            pending.len()
        ));
        let Ok(bytes) = session.player_file_refs[uid].dps_bytes() else {
            continue;
        };
        let result = bytes
            .and_then(|bytes| parse_palworld_save(&bytes).ok())
            .map(|save| scan(&save, *uid, catalogs, game_data))
            .unwrap_or_default();
        session.dps_scans.insert(*uid, result);
    }
}

pub(crate) fn scan(
    save: &Save,
    owner: uuid::Uuid,
    catalogs: &OverviewCatalogs,
    game_data: &GameData,
) -> DpsScan {
    let mut result = DpsScan::default();
    for pal in pals_in(save) {
        result.pal_count += 1;
        let codes = detect_pal_issues(pal.save_parameter, pal.character_id, catalogs);
        if !codes.is_empty() {
            result.flagged.push(flagged_row(
                pal.instance_id,
                Some(owner),
                SOURCE_DPS,
                pal.character_id,
                canonical_character_key(pal.character_id, game_data),
                current_level(pal.save_parameter),
                codes,
            ));
        }
    }
    result
}

struct DpsPal<'a> {
    save_parameter: &'a Properties,
    character_id: &'a str,
    instance_id: uuid::Uuid,
}

/// Same slot shape as [`crate::domain::pal::pal_dto_from_dps_slot`]; empty
/// slots carry `CharacterID` `"None"`.
fn pals_in(save: &Save) -> impl Iterator<Item = DpsPal<'_>> {
    save.root
        .properties
        .0
        .get(&PropertyKey::from("SaveParameterArray"))
        .and_then(props::struct_values)
        .into_iter()
        .flatten()
        .filter_map(|slot| {
            let StructValue::Struct(slot_props) = slot else {
                return None;
            };
            let save_parameter =
                props::struct_props(slot_props.0.get(&PropertyKey::from("SaveParameter"))?)?;
            let character_id = save_parameter
                .0
                .get(&PropertyKey::from("CharacterID"))
                .and_then(props::as_str)?;
            if character_id.is_empty() || character_id == "None" {
                return None;
            }
            let instance_id = slot_props
                .0
                .get(&PropertyKey::from("InstanceId"))
                .and_then(props::struct_props)
                .and_then(|inner| inner.0.get(&PropertyKey::from("InstanceId")))
                .and_then(props::as_uuid)
                .unwrap_or_default();
            Some(DpsPal {
                save_parameter,
                character_id,
                instance_id,
            })
        })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::ue::{Byte, Property, ValueVec};

    fn guid_property(text: &str) -> Property {
        Property::Struct(StructValue::Guid(
            serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap(),
        ))
    }

    pub(crate) fn slot(character_id: &str, instance_id: &str, level: u8) -> StructValue {
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", Property::Str(character_id.to_string()));
        save_parameter.insert("Level", Property::Byte(Byte::Byte(level)));
        let mut slot_props = Properties::default();
        slot_props.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(save_parameter)),
        );
        let mut inner = Properties::default();
        inner.insert("InstanceId", guid_property(instance_id));
        slot_props.insert("InstanceId", Property::Struct(StructValue::Struct(inner)));
        StructValue::Struct(slot_props)
    }

    pub(crate) fn dps_save(slots: Vec<StructValue>) -> Save {
        let mut root_properties = Properties::default();
        root_properties.insert(
            "SaveParameterArray",
            Property::Array(ValueVec::Struct(slots)),
        );
        Save {
            header: crate::ue::Header {
                magic: 0,
                save_game_version: 0,
                package_version: crate::ue::PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: crate::ue::PropertySchemas::default(),
            root: crate::ue::Root {
                save_game_type: String::new(),
                properties: root_properties,
            },
            extra: Vec::new(),
        }
    }

    #[test]
    fn walks_slots_and_skips_empty_and_malformed_ones() {
        let save = dps_save(vec![
            slot("Sheepball", "aaaaaaaa-0000-0000-0000-000000000001", 30),
            slot("None", "aaaaaaaa-0000-0000-0000-000000000002", 1),
            StructValue::Guid(crate::ue::FGuid::nil()),
        ]);
        let pals: Vec<DpsPal> = pals_in(&save).collect();
        assert_eq!(pals.len(), 1);
        assert_eq!(pals[0].character_id, "Sheepball");
        assert_eq!(
            pals[0].instance_id,
            uuid::Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000001").unwrap()
        );
    }

    #[test]
    fn a_save_without_the_array_is_empty() {
        let mut save = dps_save(vec![]);
        assert_eq!(pals_in(&save).count(), 0);
        let mut root_properties = Properties::default();
        root_properties.insert("Unrelated", Property::Str("x".into()));
        save.root.properties = root_properties;
        assert_eq!(pals_in(&save).count(), 0);
    }
}
