//! Raw-data inspector: locates one already-chosen subtree of the loaded
//! `Level.sav` and serializes it as-is. Target selection (and its priority
//! order) is the caller's job.

use crate::props;
use crate::session::SaveSession;
use uuid::Uuid;

use super::world;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawTarget {
    Guild(Uuid),
    Player(Uuid),
    Pal(Uuid),
    Base(Uuid),
    ItemContainer(Uuid),
    CharacterContainer(Uuid),
    Level,
}

impl SaveSession {
    /// `None` when the target's id doesn't resolve against this save.
    ///
    /// Each id-addressed variant re-derives its position index fresh rather
    /// than reading `self.character_index` and friends, which can be stale
    /// between a mutation and the next `rebuild_player_caches` call.
    pub fn raw_json_for(&self, target: RawTarget) -> Option<serde_json::Value> {
        match target {
            RawTarget::Guild(id) => {
                let entries = world::group_map(&self.level).ok()?;
                let entry = entries
                    .iter()
                    .find(|entry| props::as_uuid(&entry.key) == Some(id))?;
                serde_json::to_value(entry).ok()
            }
            // A player's entry is addressed by PlayerUId + IsPlayer, not by
            // character_index, which keys by InstanceId and would resolve an
            // unrelated pal.
            RawTarget::Player(id) => {
                let entries = world::character_map(&self.level).ok()?;
                let entry = entries.iter().find(|entry| {
                    world::entry_is_player(entry) && world::entry_player_uid(entry) == Some(id)
                })?;
                serde_json::to_value(entry).ok()
            }
            RawTarget::Pal(id) => {
                let index = world::build_character_index(&self.level);
                let position = *index.get(&id)?;
                let entries = world::character_map(&self.level).ok()?;
                serde_json::to_value(entries.get(position)?).ok()
            }
            RawTarget::Base(id) => {
                let entries = world::base_camp_map(&self.level).ok().flatten()?;
                let entry = entries
                    .iter()
                    .find(|entry| props::as_uuid(&entry.key) == Some(id))?;
                serde_json::to_value(entry).ok()
            }
            RawTarget::ItemContainer(id) => {
                let index = world::build_item_container_index(&self.level);
                let position = *index.get(&id)?;
                let entries = world::item_container_map(&self.level).ok()?;
                serde_json::to_value(entries.get(position)?).ok()
            }
            RawTarget::CharacterContainer(id) => {
                let index = world::build_character_container_index(&self.level);
                let position = *index.get(&id)?;
                let entries = world::character_container_map(&self.level).ok()?;
                serde_json::to_value(entries.get(position)?).ok()
            }
            // The GVAS root only: `Save`'s header/schemas/extra are
            // (de)serializer plumbing, not save data.
            RawTarget::Level => serde_json::to_value(&self.level.root).ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SaveKind;
    use crate::ue::games::palworld::PalCharacterData;
    use crate::ue::{
        Header, MapEntry, PackageVersion, Properties, Property, PropertySchemas, Root, Save,
        StructValue,
    };

    const GUILD_ID: &str = "33333333-3333-3333-3333-333333333333";
    const PLAYER_ID: &str = "11111111-1111-1111-1111-111111111111";
    const PAL_ID: &str = "22222222-2222-2222-2222-222222222222";
    const BASE_ID: &str = "44444444-4444-4444-4444-444444444444";
    const ITEM_CONTAINER_ID: &str = "55555555-5555-5555-5555-555555555555";
    const CHARACTER_CONTAINER_ID: &str = "66666666-6666-6666-6666-666666666666";
    const UNKNOWN_ID: &str = "99999999-9999-9999-9999-999999999999";

    fn uid(text: &str) -> Uuid {
        text.parse().unwrap()
    }

    fn struct_property(properties: Properties) -> Property {
        Property::Struct(StructValue::Struct(properties))
    }

    fn minimal_save(properties: Properties) -> Save {
        Save {
            header: Header {
                magic: 0,
                save_game_version: 0,
                package_version: PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: PropertySchemas::default(),
            root: Root {
                save_game_type: "TestSaveGame".to_string(),
                properties,
            },
            extra: Vec::new(),
        }
    }

    /// `PlayerUId`/`InstanceId` in the key, `IsPlayer` inside
    /// `RawData.object.SaveParameter`.
    fn character_entry(player_uid: &str, instance_id: &str, is_player: bool) -> MapEntry {
        let mut key_properties = Properties::default();
        key_properties.insert("PlayerUId", props::guid_property(uid(player_uid)));
        key_properties.insert("InstanceId", props::guid_property(uid(instance_id)));

        let mut save_parameter = Properties::default();
        save_parameter.insert("IsPlayer", Property::Bool(is_player));
        let mut object = Properties::default();
        object.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(save_parameter)),
        );
        let character_data = PalCharacterData {
            object,
            unknown_bytes: [0; 4],
            group_id: crate::ue::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterData(character_data))),
        );

        MapEntry {
            key: struct_property(key_properties),
            value: struct_property(value_properties),
        }
    }

    fn keyed_id_entry(id_text: &str) -> MapEntry {
        let mut key_properties = Properties::default();
        key_properties.insert("ID", props::guid_property(uid(id_text)));
        MapEntry {
            key: struct_property(key_properties),
            value: Property::Bool(true),
        }
    }

    fn guild_entry(guild_id: &str) -> MapEntry {
        MapEntry {
            key: props::guid_property(uid(guild_id)),
            value: Property::Bool(true),
        }
    }

    fn base_entry(base_id: &str) -> MapEntry {
        MapEntry {
            key: props::guid_property(uid(base_id)),
            value: Property::Bool(true),
        }
    }

    /// One entry of each kind -- enough for every `RawTarget` variant to
    /// resolve against a single fixture.
    fn session_with_every_target() -> SaveSession {
        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "GroupSaveDataMap",
            Property::Map(vec![guild_entry(GUILD_ID)]),
        );
        world_save_data.insert(
            "CharacterSaveParameterMap",
            Property::Map(vec![
                character_entry(PLAYER_ID, PLAYER_ID, true),
                character_entry(PLAYER_ID, PAL_ID, false),
            ]),
        );
        world_save_data.insert("BaseCampSaveData", Property::Map(vec![base_entry(BASE_ID)]));
        world_save_data.insert(
            "ItemContainerSaveData",
            Property::Map(vec![keyed_id_entry(ITEM_CONTAINER_ID)]),
        );
        world_save_data.insert(
            "CharacterContainerSaveData",
            Property::Map(vec![keyed_id_entry(CHARACTER_CONTAINER_ID)]),
        );

        let mut root_properties = Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        SaveSession::new_for_tests(SaveKind::InMemory, minimal_save(root_properties))
    }

    fn assert_resolves_to_non_empty_object(value: Option<serde_json::Value>) {
        let object = value
            .expect("target should resolve")
            .as_object()
            .expect("resolved JSON must be an object")
            .clone();
        assert!(!object.is_empty(), "resolved JSON object must not be empty");
    }

    #[test]
    fn raw_json_for_guild_resolves_the_matching_entry() {
        let session = session_with_every_target();
        assert_resolves_to_non_empty_object(session.raw_json_for(RawTarget::Guild(uid(GUILD_ID))));
        assert!(session
            .raw_json_for(RawTarget::Guild(uid(UNKNOWN_ID)))
            .is_none());
    }

    #[test]
    fn raw_json_for_player_requires_both_is_player_and_matching_uid() {
        let session = session_with_every_target();
        assert_resolves_to_non_empty_object(
            session.raw_json_for(RawTarget::Player(uid(PLAYER_ID))),
        );
        // The pal entry's InstanceId must not resolve as a Player target.
        assert!(session
            .raw_json_for(RawTarget::Player(uid(PAL_ID)))
            .is_none());
        assert!(session
            .raw_json_for(RawTarget::Player(uid(UNKNOWN_ID)))
            .is_none());
    }

    #[test]
    fn raw_json_for_pal_resolves_by_instance_id_regardless_of_is_player() {
        let session = session_with_every_target();
        assert_resolves_to_non_empty_object(session.raw_json_for(RawTarget::Pal(uid(PAL_ID))));
        assert!(session
            .raw_json_for(RawTarget::Pal(uid(UNKNOWN_ID)))
            .is_none());
    }

    #[test]
    fn raw_json_for_base_resolves_the_matching_entry() {
        let session = session_with_every_target();
        assert_resolves_to_non_empty_object(session.raw_json_for(RawTarget::Base(uid(BASE_ID))));
        assert!(session
            .raw_json_for(RawTarget::Base(uid(UNKNOWN_ID)))
            .is_none());
    }

    #[test]
    fn raw_json_for_base_is_none_when_base_camp_save_data_is_entirely_absent() {
        // A world that has never built a base carries no BaseCampSaveData.
        let mut root_properties = Properties::default();
        root_properties.insert("worldSaveData", struct_property(Properties::default()));
        let session = SaveSession::new_for_tests(SaveKind::InMemory, minimal_save(root_properties));
        assert!(session
            .raw_json_for(RawTarget::Base(uid(BASE_ID)))
            .is_none());
    }

    #[test]
    fn raw_json_for_item_container_resolves_the_matching_entry() {
        let session = session_with_every_target();
        assert_resolves_to_non_empty_object(
            session.raw_json_for(RawTarget::ItemContainer(uid(ITEM_CONTAINER_ID))),
        );
        assert!(session
            .raw_json_for(RawTarget::ItemContainer(uid(UNKNOWN_ID)))
            .is_none());
    }

    #[test]
    fn raw_json_for_character_container_resolves_the_matching_entry() {
        let session = session_with_every_target();
        assert_resolves_to_non_empty_object(
            session.raw_json_for(RawTarget::CharacterContainer(uid(CHARACTER_CONTAINER_ID))),
        );
        assert!(session
            .raw_json_for(RawTarget::CharacterContainer(uid(UNKNOWN_ID)))
            .is_none());
    }

    #[test]
    fn raw_json_for_level_always_resolves_to_the_gvas_root() {
        let session = session_with_every_target();
        let value = session
            .raw_json_for(RawTarget::Level)
            .expect("Level always resolves");
        assert_eq!(value["save_game_type"], "TestSaveGame");
        assert!(value["properties"].is_object());
    }
}
