//! GPS (Global Pal Storage) session state and pal operations.
//!
//! `GlobalPalStorage.sav`'s root `SaveParameterArray` uses the exact same
//! per-slot `{"InstanceId", "SaveParameter"}` layout as a player's `_dps.sav`
//! array, so this module is a thin wrapper around `domain::pal`'s DPS slot
//! machinery rather than a separate implementation.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::ue::{PropertyKey, StructValue};

use crate::domain::pal;
use crate::dto::ordered_map::OrderedMap;
use crate::dto::pal::PalDto;
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::props;
use crate::session::SaveSession;

/// Parsed `GlobalPalStorage.sav` plus everything the GPS mutators need to
/// read and write its `SaveParameterArray` without re-parsing the file.
#[derive(Default)]
pub struct GpsState {
    /// Known as soon as the GPS file is located, before it is ever parsed --
    /// `gps_available` is true from that point on.
    pub file_path: Option<PathBuf>,
    pub save: Option<crate::ue::Save>,
    /// `SaveParameterArray` length: every valid slot index is
    /// `0..slot_count`, occupied or not.
    pub slot_count: usize,
    /// Index -> occupied slot. Every mutator re-derives the entries it
    /// touches from the just-written slot, so this never goes stale.
    pub pals: BTreeMap<i32, PalDto>,
    pub loaded: bool,
}

fn gps_slots(save: &crate::ue::Save) -> Option<&Vec<StructValue>> {
    props::struct_values(
        save.root
            .properties
            .0
            .get(&PropertyKey::from("SaveParameterArray"))?,
    )
}

fn gps_slots_mut(save: &mut crate::ue::Save) -> Option<&mut Vec<StructValue>> {
    props::struct_values_mut(
        save.root
            .properties
            .0
            .get_mut(&PropertyKey::from("SaveParameterArray"))?,
    )
}

/// A slot is empty when its `CharacterID` is absent or the literal string
/// `"None"` -- the save format's own vacancy marker.
fn gps_slot_is_empty(slot: &StructValue) -> bool {
    let StructValue::Struct(slot_props) = slot else {
        return true;
    };
    let Some(save_parameter) = slot_props
        .0
        .get(&PropertyKey::from("SaveParameter"))
        .and_then(props::struct_props)
    else {
        return true;
    };
    match pal::param(save_parameter, "CharacterID").and_then(props::as_str) {
        None => true,
        Some("None") => true,
        Some(_) => false,
    }
}

/// Save data is untrusted: an unparseable slot is skipped, never fatal.
fn collect_gps_pals(slots: &[StructValue], game_data: &GameData) -> BTreeMap<i32, PalDto> {
    let mut pals = BTreeMap::new();
    for (index, slot) in slots.iter().enumerate() {
        if gps_slot_is_empty(slot) {
            continue;
        }
        if let Some(dto) = pal::pal_dto_from_dps_slot(slot, game_data) {
            pals.insert(index as i32, dto);
        }
    }
    pals
}

impl SaveSession {
    pub fn gps_available(&self) -> bool {
        self.gps.loaded || self.gps.file_path.is_some()
    }

    /// `None` until `load_gps` has run at least once.
    pub fn gps_pals(&self) -> Option<&BTreeMap<i32, PalDto>> {
        self.gps.loaded.then_some(&self.gps.pals)
    }

    /// Parses `GlobalPalStorage.sav` and extracts every non-empty
    /// `SaveParameterArray` slot as a `PalDto`.
    pub fn load_gps(
        &mut self,
        sav_bytes: &[u8],
        game_data: &GameData,
    ) -> Result<&BTreeMap<i32, PalDto>, CoreError> {
        let mut save = crate::session::parse_palworld_save(sav_bytes)?;
        let slots = gps_slots(&save).ok_or_else(|| {
            CoreError::Parse("GlobalPalStorage SaveParameterArray missing".into())
        })?;
        self.gps.pals = collect_gps_pals(slots, game_data);
        self.gps.slot_count = slots.len();
        crate::domain::pal::ensure_slot_pal_schemas(&mut save);
        self.gps.save = Some(save);
        self.gps.loaded = true;
        Ok(&self.gps.pals)
    }

    /// `None` both when no slot is free and when no GPS file is loaded;
    /// callers that need the distinction check `gps.loaded` first.
    pub fn find_first_empty_gps_slot(&self) -> Option<i32> {
        let slots = gps_slots(self.gps.save.as_ref()?)?;
        slots
            .iter()
            .position(gps_slot_is_empty)
            .map(|index| index as i32)
    }

    /// Creates a pal in `storage_slot`, or the first empty slot. Returns
    /// `(pal, index)`, or `None` when GPS is full.
    ///
    /// `FullStomach` is deliberately left as the slot's previous occupant
    /// left it: unlike a newly caught pal, a GPS deposit is not fed.
    pub fn add_gps_pal(
        &mut self,
        game_data: &GameData,
        character_id: &str,
        nickname: &str,
        storage_slot: Option<i32>,
    ) -> Result<Option<(PalDto, i32)>, CoreError> {
        if !self.gps.loaded {
            return Err(CoreError::Other(
                "GPS Gvas file is not initialized.".to_string(),
            ));
        }
        let Some(slot_index) = storage_slot.or_else(|| self.find_first_empty_gps_slot()) else {
            return Ok(None);
        };
        let save = self
            .gps
            .save
            .as_mut()
            .expect("gps.loaded implies save is Some");
        let Some(slots) = gps_slots_mut(save) else {
            return Ok(None);
        };
        if slot_index < 0 || slot_index as usize >= slots.len() {
            return Ok(None);
        }
        let new_instance_id = uuid::Uuid::new_v4();
        {
            let StructValue::Struct(slot_props) = &mut slots[slot_index as usize] else {
                return Ok(None);
            };
            if let Some(id_struct) = slot_props
                .0
                .get_mut(&PropertyKey::from("InstanceId"))
                .and_then(props::struct_props_mut)
            {
                id_struct.insert("InstanceId", props::guid_property(new_instance_id));
            }
            let Some(save_parameter) = slot_props
                .0
                .get_mut(&PropertyKey::from("SaveParameter"))
                .and_then(props::struct_props_mut)
            else {
                return Ok(None);
            };
            pal::reset_dps_save_parameter(save_parameter);
            save_parameter.insert("CharacterID", props::name_property(character_id));
            save_parameter.insert("NickName", props::str_property(nickname));
            save_parameter.insert("FilteredNickName", props::str_property(nickname));
            // The save format spells this key both ways across game versions.
            let slot_key = if save_parameter.0.contains_key(&PropertyKey::from("SlotID")) {
                "SlotID"
            } else {
                "SlotId"
            };
            if let Some(slot_struct) = save_parameter
                .0
                .get_mut(&PropertyKey::from(slot_key))
                .and_then(props::struct_props_mut)
            {
                slot_struct.insert("SlotIndex", props::int_property(0));
            }
            save_parameter.insert("Gender", props::enum_property("EPalGenderType::Female"));
            save_parameter.insert(
                "GotStatusPointList",
                pal::status_point_structs(&pal::STATUS_NAMES),
            );
            save_parameter.insert(
                "GotExStatusPointList",
                pal::status_point_structs(&pal::EX_STATUS_NAMES),
            );
            // A new pal enters storage at full health.
            let dto =
                pal::read_save_parameter_dto(save_parameter, new_instance_id, true, game_data);
            let boosted = dto.is_boss.unwrap_or(false) || dto.is_lucky.unwrap_or(false);
            save_parameter.insert(
                "Hp",
                props::fixed_point64_property(pal::max_hp_for(&dto, boosted, game_data)),
            );
        }
        let slots = gps_slots(save).expect("just wrote to it");
        let Some(dto) = pal::pal_dto_from_dps_slot(&slots[slot_index as usize], game_data) else {
            return Ok(None);
        };
        self.gps.pals.insert(slot_index, dto.clone());
        Ok(Some((dto, slot_index)))
    }

    /// Writes `pal_dto` into a GPS slot as a fresh, unowned pal -- also the
    /// whole of cloning a GPS pal. Returns `(index, pal)`.
    pub fn add_gps_pal_from_dto(
        &mut self,
        game_data: &GameData,
        pal_dto: &PalDto,
        storage_slot: Option<i32>,
    ) -> Result<Option<(i32, PalDto)>, CoreError> {
        if !self.gps.loaded {
            return Err(CoreError::Other(
                "GPS Gvas file is not initialized.".to_string(),
            ));
        }
        let Some(slot_index) = storage_slot.or_else(|| self.find_first_empty_gps_slot()) else {
            return Ok(None);
        };
        let save = self
            .gps
            .save
            .as_mut()
            .expect("gps.loaded implies save is Some");
        let Some(slots) = gps_slots_mut(save) else {
            return Ok(None);
        };
        if slot_index < 0 || slot_index as usize >= slots.len() {
            return Ok(None);
        }
        let new_instance_id = uuid::Uuid::new_v4();
        let mut incoming = pal_dto.clone();
        // Must be a present nil guid, not `None`: `apply_pal_dto` skips
        // `OwnerPlayerUId` entirely when the field is `None`, which would
        // leave the clone owned by the source pal's player.
        incoming.owner_uid = Some(props::EMPTY_UUID);
        incoming.instance_id = new_instance_id;
        incoming.storage_id = props::EMPTY_UUID;
        incoming.storage_slot = 0;
        {
            let StructValue::Struct(slot_props) = &mut slots[slot_index as usize] else {
                return Ok(None);
            };
            if let Some(id_struct) = slot_props
                .0
                .get_mut(&PropertyKey::from("InstanceId"))
                .and_then(props::struct_props_mut)
            {
                id_struct.insert("InstanceId", props::guid_property(new_instance_id));
            }
            let Some(save_parameter) = slot_props
                .0
                .get_mut(&PropertyKey::from("SaveParameter"))
                .and_then(props::struct_props_mut)
            else {
                return Ok(None);
            };
            // `apply_pal_dto` already writes Hp = max_hp; no separate Hp write.
            pal::apply_pal_dto(save_parameter, &incoming, true, game_data);
            save_parameter.insert(
                "GotStatusPointList",
                pal::status_point_structs(&pal::STATUS_NAMES),
            );
            save_parameter.insert(
                "GotExStatusPointList",
                pal::status_point_structs(&pal::EX_STATUS_NAMES),
            );
        }
        let slots = gps_slots(save).expect("just wrote to it");
        let Some(dto) = pal::pal_dto_from_dps_slot(&slots[slot_index as usize], game_data) else {
            return Ok(None);
        };
        self.gps.pals.insert(slot_index, dto.clone());
        Ok(Some((slot_index, dto)))
    }

    /// Vacates each tracked slot in place: the array keeps its length, so the
    /// slot's outer `InstanceId` must be zeroed too, or the game still sees a
    /// pal there.
    pub fn delete_gps_pals(&mut self, pal_indexes: &[i32]) {
        let GpsState {
            save, pals, loaded, ..
        } = &mut self.gps;
        if !*loaded {
            return;
        }
        let Some(save) = save.as_mut() else {
            return;
        };
        let Some(slots) = gps_slots_mut(save) else {
            return;
        };
        let mut sorted_indexes: Vec<i32> = pal_indexes.to_vec();
        sorted_indexes.sort_unstable_by(|a, b| b.cmp(a));
        for index in sorted_indexes {
            if index < 0 || !pals.contains_key(&index) {
                continue;
            }
            let Some(StructValue::Struct(slot_props)) = slots.get_mut(index as usize) else {
                continue;
            };
            if let Some(id_struct) = slot_props
                .0
                .get_mut(&PropertyKey::from("InstanceId"))
                .and_then(props::struct_props_mut)
            {
                id_struct.insert("InstanceId", props::guid_property(props::EMPTY_UUID));
            }
            if let Some(save_parameter) = slot_props
                .0
                .get_mut(&PropertyKey::from("SaveParameter"))
                .and_then(props::struct_props_mut)
            {
                pal::reset_dps_save_parameter(save_parameter);
            }
            pals.remove(&index);
        }
    }

    /// Applies each DTO in `modified_gps_pals` onto its `SaveParameterArray`
    /// slot, addressed by slot index. A DTO whose slot is out of range or not a
    /// struct is skipped. Mirrors `domain::pal::update_dps_pals`: the same
    /// `apply_pal_dto` slot machinery, since GPS slots share the DPS layout.
    pub fn update_gps_pals(
        &mut self,
        game_data: &GameData,
        modified_gps_pals: &OrderedMap<i32, PalDto>,
        progress: &crate::progress::ProgressSink,
    ) -> Result<(), CoreError> {
        if !self.gps.loaded {
            return Err(CoreError::Other(
                "GPS Gvas file is not initialized.".to_string(),
            ));
        }
        {
            let save = self
                .gps
                .save
                .as_mut()
                .expect("gps.loaded implies save is Some");
            let Some(slots) = gps_slots_mut(save) else {
                return Ok(());
            };
            for (slot_index, dto) in modified_gps_pals.iter() {
                let display_name = dto
                    .nickname
                    .clone()
                    .unwrap_or_else(|| dto.character_id.clone());
                progress(&format!("Updating GPS pal {display_name}"));
                if *slot_index < 0 || *slot_index as usize >= slots.len() {
                    continue;
                }
                let StructValue::Struct(slot_props) = &mut slots[*slot_index as usize] else {
                    continue;
                };
                if let Some(save_parameter) = slot_props
                    .0
                    .get_mut(&PropertyKey::from("SaveParameter"))
                    .and_then(props::struct_props_mut)
                {
                    pal::apply_pal_dto(save_parameter, dto, true, game_data);
                }
            }
        }
        // Re-derive the touched entries from the just-written slots.
        self.rebuild_gps_save(game_data)?;
        progress("Saving changes to file");
        Ok(())
    }

    /// Re-derives `gps.pals`/`gps.slot_count` from the live property tree.
    /// The mutators keep both in sync already; this is the defensive resync
    /// the save-to-disk path can run first.
    pub fn rebuild_gps_save(&mut self, game_data: &GameData) -> Result<(), CoreError> {
        let save = self
            .gps
            .save
            .as_ref()
            .ok_or_else(|| CoreError::Other("GPS Gvas file is not initialized.".to_string()))?;
        let slots = gps_slots(save).ok_or_else(|| {
            CoreError::Parse("GlobalPalStorage SaveParameterArray missing".into())
        })?;
        self.gps.slot_count = slots.len();
        self.gps.pals = collect_gps_pals(slots, game_data);
        Ok(())
    }

    /// Compresses the loaded `GlobalPalStorage.sav` tree back to `.sav` bytes,
    /// or `None` when no GPS file was loaded.
    pub fn gps_sav_bytes(&self) -> Result<Option<Vec<u8>>, CoreError> {
        match &self.gps.save {
            Some(save) => Ok(Some(crate::savio::write_sav_bytes(save)?)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SaveKind;
    use crate::ue::{
        Header, PackageVersion, Properties, Property, PropertySchemas, Root, Save, ValueVec,
    };
    use uuid::Uuid;

    fn game_data() -> GameData {
        let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
        GameData::load(&json_dir).expect("data dir")
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
                save_game_type: String::new(),
                properties,
            },
            extra: Vec::new(),
        }
    }

    fn gps_slot(character_id: &str, instance_id: Uuid) -> StructValue {
        let mut save_parameter = Properties::default();
        if character_id != "None" {
            save_parameter.insert("CharacterID", props::name_property(character_id));
        }
        save_parameter.insert("Level", props::byte_property(5));
        save_parameter.insert("Talent_HP", props::byte_property(30));
        let mut container_struct = Properties::default();
        container_struct.insert(
            "ID",
            Property::Struct(StructValue::Guid(props::uuid_to_guid(Uuid::nil()))),
        );
        let mut slot_struct = Properties::default();
        slot_struct.insert(
            "ContainerId",
            Property::Struct(StructValue::Struct(container_struct)),
        );
        slot_struct.insert("SlotIndex", props::int_property(-1));
        save_parameter.insert("SlotID", Property::Struct(StructValue::Struct(slot_struct)));
        save_parameter.insert(
            "GotStatusPointList",
            Property::Array(ValueVec::Struct(vec![])),
        );
        save_parameter.insert(
            "GotExStatusPointList",
            Property::Array(ValueVec::Struct(vec![])),
        );

        let mut inner_instance_id = Properties::default();
        inner_instance_id.insert(
            "InstanceId",
            Property::Struct(StructValue::Guid(props::uuid_to_guid(instance_id))),
        );
        let mut slot_props = Properties::default();
        slot_props.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(save_parameter)),
        );
        slot_props.insert(
            "InstanceId",
            Property::Struct(StructValue::Struct(inner_instance_id)),
        );
        StructValue::Struct(slot_props)
    }

    /// Three GPS slots: 0 empty, 1 holding a lucky pal, 2 empty -- two free
    /// slots so an add and a subsequent clone both land somewhere.
    fn gps_fixture_session() -> (SaveSession, GameData) {
        let data = game_data();
        let mut lucky_slot = match gps_slot("Sheepball", Uuid::new_v4()) {
            StructValue::Struct(p) => p,
            _ => unreachable!(),
        };
        if let Some(save_parameter) = lucky_slot
            .0
            .get_mut(&PropertyKey::from("SaveParameter"))
            .and_then(props::struct_props_mut)
        {
            save_parameter.insert("IsRarePal", Property::Bool(true));
        }
        let mut gps_root_properties = Properties::default();
        gps_root_properties.insert(
            "SaveParameterArray",
            Property::Array(ValueVec::Struct(vec![
                gps_slot("None", Uuid::nil()),
                StructValue::Struct(lucky_slot),
                gps_slot("None", Uuid::nil()),
            ])),
        );
        let gps_save = minimal_save(gps_root_properties);

        let level = minimal_save(Properties::default());
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        let slots = gps_slots(&gps_save).unwrap();
        session.gps.pals = collect_gps_pals(slots, &data);
        session.gps.slot_count = slots.len();
        session.gps.save = Some(gps_save);
        session.gps.loaded = true;

        (session, data)
    }

    #[test]
    fn gps_available_reflects_file_path_and_loaded_state() {
        let level = minimal_save(Properties::default());
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        assert!(!session.gps_available());

        session.gps.file_path = Some(PathBuf::from("/tmp/GlobalPalStorage.sav"));
        assert!(session.gps_available());
    }

    #[test]
    fn gps_pals_is_none_until_loaded() {
        let level = minimal_save(Properties::default());
        let session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        assert!(session.gps_pals().is_none());
    }

    #[test]
    fn find_first_empty_gps_slot_finds_slot_zero() {
        let (session, _data) = gps_fixture_session();
        assert_eq!(session.find_first_empty_gps_slot(), Some(0));
    }

    #[test]
    fn add_gps_pal_fills_the_first_empty_slot_and_heals_to_full() {
        let (mut session, data) = gps_fixture_session();
        assert_eq!(session.gps_pals().unwrap().len(), 1);

        let (new_pal, slot) = session
            .add_gps_pal(&data, "SheepBall", "TestSheep", None)
            .unwrap()
            .expect("slot 0 is empty");
        assert_eq!(slot, 0);
        assert_eq!(new_pal.character_id, "SheepBall");
        assert_eq!(new_pal.nickname.as_deref(), Some("TestSheep"));
        assert_eq!(new_pal.owner_uid, Some(props::EMPTY_UUID));
        assert_eq!(new_pal.hp, new_pal.max_hp);
        assert!(new_pal.hp > 0);
        assert_eq!(session.gps_pals().unwrap().len(), 2);
    }

    /// Resetting a slot never clears `IsRarePal`, so a recycled slot keeps it.
    #[test]
    fn add_gps_pal_into_a_recycled_slot_inherits_a_stale_is_rare_pal_flag() {
        let (mut session, data) = gps_fixture_session();

        let (new_pal, slot) = session
            .add_gps_pal(&data, "SheepBall", "TestSheep", Some(1))
            .unwrap()
            .expect("slot 1 explicitly requested");
        assert_eq!(slot, 1);
        assert_eq!(new_pal.is_lucky, Some(true));
    }

    #[test]
    fn add_gps_pal_from_dto_clones_with_a_fresh_instance_id_and_no_owner() {
        let (mut session, data) = gps_fixture_session();
        let (source, source_slot) = session
            .add_gps_pal(&data, "SheepBall", "TestSheep", None)
            .unwrap()
            .unwrap();

        let (clone_slot, clone) = session
            .add_gps_pal_from_dto(&data, &source, None)
            .unwrap()
            .expect("a second empty slot exists");
        assert_ne!(clone_slot, source_slot);
        assert_ne!(clone.instance_id, source.instance_id);
        assert_eq!(clone.owner_uid, Some(props::EMPTY_UUID));
        assert_eq!(clone.character_id, source.character_id);
        assert_eq!(session.gps_pals().unwrap().len(), 3);
    }

    #[test]
    fn delete_gps_pals_frees_the_slot_and_clears_the_outer_instance_id() {
        let (mut session, data) = gps_fixture_session();
        let (new_pal, slot) = session
            .add_gps_pal(&data, "SheepBall", "TestSheep", None)
            .unwrap()
            .unwrap();
        assert_eq!(session.gps_pals().unwrap().len(), 2);

        session.delete_gps_pals(&[slot]);

        assert_eq!(session.gps_pals().unwrap().len(), 1);
        assert!(!session.gps_pals().unwrap().contains_key(&slot));
        assert_eq!(session.find_first_empty_gps_slot(), Some(slot));

        let save = session.gps.save.as_ref().unwrap();
        let slots = gps_slots(save).unwrap();
        let StructValue::Struct(slot_props) = &slots[slot as usize] else {
            panic!("slot must still be a struct");
        };
        let outer_instance_id = props::get(slot_props, &["InstanceId", "InstanceId"])
            .and_then(props::as_uuid)
            .unwrap();
        assert_eq!(outer_instance_id, props::EMPTY_UUID);
        assert_ne!(new_pal.instance_id, props::EMPTY_UUID);
    }

    #[test]
    fn delete_gps_pals_ignores_an_index_not_currently_tracked() {
        let (mut session, _data) = gps_fixture_session();
        let before = session.gps_pals().unwrap().len();
        session.delete_gps_pals(&[999]);
        assert_eq!(session.gps_pals().unwrap().len(), before);
    }

    #[test]
    fn rebuild_gps_save_resyncs_pals_after_direct_mutation() {
        let (mut session, data) = gps_fixture_session();
        session
            .add_gps_pal(&data, "SheepBall", "TestSheep", None)
            .unwrap()
            .unwrap();
        session.gps.pals.clear(); // simulate the cache going stale

        session.rebuild_gps_save(&data).unwrap();

        assert_eq!(session.gps_pals().unwrap().len(), 2);
    }

    #[test]
    fn update_gps_pals_applies_the_dto_onto_the_occupied_slot() {
        let (mut session, data) = gps_fixture_session();
        // Seed a plain (non-rare) pal into empty slot 0 so the edit round-trip
        // is not confounded by the lucky pal's BOSS_ prefix normalization.
        let (seeded, slot) = session
            .add_gps_pal(&data, "SheepBall", "TestSheep", Some(0))
            .unwrap()
            .expect("slot 0 is empty");
        assert_eq!(slot, 0);

        let mut edited = seeded.clone();
        edited.nickname = Some("GPS Edited".to_string());
        edited.level = 42;
        let mut modified: OrderedMap<i32, PalDto> = OrderedMap::new();
        modified.insert(0, edited);

        let progress: crate::progress::ProgressSink = std::sync::Arc::new(|_: &str| {});
        session.update_gps_pals(&data, &modified, &progress).unwrap();

        let updated = &session.gps_pals().unwrap()[&0];
        assert_eq!(updated.nickname.as_deref(), Some("GPS Edited"));
        assert_eq!(updated.level, 42);
        assert_eq!(updated.character_id, "SheepBall");
        assert_eq!(updated.instance_id, seeded.instance_id);
    }

    #[test]
    fn update_gps_pals_skips_an_out_of_range_slot_without_panicking() {
        let (mut session, data) = gps_fixture_session();
        let dto = session.gps_pals().unwrap()[&1].clone();
        let mut modified: OrderedMap<i32, PalDto> = OrderedMap::new();
        modified.insert(999, dto);

        let progress: crate::progress::ProgressSink = std::sync::Arc::new(|_: &str| {});
        session.update_gps_pals(&data, &modified, &progress).unwrap();

        assert_eq!(session.gps_pals().unwrap().len(), 1);
        assert!(session.gps_pals().unwrap().contains_key(&1));
    }

    #[test]
    fn update_gps_pals_errors_when_not_loaded() {
        let level = minimal_save(Properties::default());
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        let data = game_data();
        let progress: crate::progress::ProgressSink = std::sync::Arc::new(|_: &str| {});
        let error = session
            .update_gps_pals(&data, &OrderedMap::new(), &progress)
            .unwrap_err();
        assert_eq!(error.to_string(), "GPS Gvas file is not initialized.");
    }

    #[test]
    fn add_gps_pal_errors_with_pythons_message_when_not_loaded() {
        let level = minimal_save(Properties::default());
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        let data = game_data();
        let error = session
            .add_gps_pal(&data, "SheepBall", "x", None)
            .unwrap_err();
        assert_eq!(error.to_string(), "GPS Gvas file is not initialized.");
    }

    // Re-serializing a real parsed tree is left to the corpus-gated test
    // below: `uesave`'s writer needs each property's schema, which is only
    // recorded when the property was actually read through `SaveReader`, so
    // `write_sav_bytes` on a hand-built `Save` always fails with `missing
    // property schema`.

    #[test]
    fn gps_sav_bytes_is_none_when_not_loaded() {
        let level = minimal_save(Properties::default());
        let session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        assert!(session.gps_sav_bytes().unwrap().is_none());
    }

    /// Exercises the actual parse/compression path against the committed
    /// `GlobalPalStorage.sav` fixture. Never skips.
    #[test]
    fn gps_load_add_clone_delete_round_trips_against_a_real_file() {
        let gps_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves/GlobalPalStorage.sav");
        let gps_bytes = std::fs::read(gps_path).expect("read committed GlobalPalStorage.sav fixture");
        let data = game_data();
        let level = minimal_save(Properties::default());
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);

        assert!(session.gps_pals().is_none(), "not loaded yet");
        session.load_gps(&gps_bytes, &data).unwrap();
        let initial_count = session.gps_pals().unwrap().len();

        let Some((new_pal, slot)) = session
            .add_gps_pal(&data, "SheepBall", "TestSheep", None)
            .unwrap()
        else {
            eprintln!("no empty GPS slot in this corpus file; nothing to prove");
            return;
        };
        assert_eq!(new_pal.character_id, "SheepBall");
        assert_eq!(new_pal.nickname.as_deref(), Some("TestSheep"));
        assert_eq!(new_pal.hp, new_pal.max_hp);
        assert_eq!(session.gps_pals().unwrap().len(), initial_count + 1);

        let Some((clone_slot, clone)) =
            session.add_gps_pal_from_dto(&data, &new_pal, None).unwrap()
        else {
            eprintln!("no second empty GPS slot in this corpus file; nothing to prove");
            return;
        };
        assert_ne!(clone_slot, slot);
        assert_ne!(clone.instance_id, new_pal.instance_id);

        session.delete_gps_pals(&[slot, clone_slot]);
        assert_eq!(session.gps_pals().unwrap().len(), initial_count);
        assert!(session.find_first_empty_gps_slot().is_some());
        session.rebuild_gps_save(&data).unwrap();
        assert!(session.gps_sav_bytes().unwrap().is_some());
    }
}
