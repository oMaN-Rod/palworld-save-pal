//! GPS (Global Pal Storage) session state and pal operations -- port of
//! `PalOpsMixin`'s GPS methods (`game/mixins/pal_ops.py:33-214,373-382`) plus
//! `SaveManager.load_gps`/`_load_gps_pals`/`get_gps`
//! (`game/mixins/loading.py:363-...`, `pal_ops.py:33-51`).
//!
//! `GlobalPalStorage.sav`'s root property IS a `SaveParameterArray`
//! (`self._gps_gvas_file.properties["SaveParameterArray"]` in Python) --
//! exactly the same per-slot `{"InstanceId": ..., "SaveParameter": ...}`
//! layout `domain::pal`'s DPS ops (`add_player_dps_pal`,
//! `pal_dto_from_dps_slot`, `reset_dps_save_parameter`, ...) already read and
//! write for a player's `_dps.sav` array. This module is deliberately a thin
//! GPS-specific wrapper around that exact machinery rather than a
//! reimplementation -- see this task's report for the full reconciliation
//! against the task brief, which assumed a since-superseded `Pal` struct
//! (`Pal::from_gps_entry`/`into_gps_entry`/`new_dps_format`/...) that does
//! not exist in this port's actual (DTO/functional, Phase 2) architecture.
//!
//! Deviation from the brief's exact `SaveSession` method signatures: every
//! method here that builds a `PalDto` from raw save data
//! (`load_gps`/`add_gps_pal`/`add_gps_pal_from_dto`/`rebuild_gps_save`) takes
//! an explicit `game_data: &GameData` parameter, which the brief's signatures
//! omit. This is not optional: `GameData` is loaded once at server startup
//! and threaded explicitly into every other pal-DTO-producing call in this
//! crate (`ctx.app.game_data` in every `psp-server/src/handlers/pals.rs`
//! handler; `pal::add_player_dps_pal(session, game_data, ...)`,
//! `pal::pal_dto_from_dps_slot(slot, game_data)`, ...) -- it is never
//! reachable from `SaveSession` itself (no such field exists, and adding one
//! would duplicate the `Arc<GameData>` the server already owns). The brief's
//! own example test calls `session.load_gps(&gps_bytes)` with no
//! `GameData` at all, which cannot build a real `PalDto` (character stats,
//! max HP, `character_key` formatting all read `pals.json`); this is the one
//! part of the brief's contract that could not be kept as literally written.

use std::collections::BTreeMap;
use std::path::PathBuf;

use uesave::{PropertyKey, StructValue};

use crate::domain::pal;
use crate::dto::pal::PalDto;
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::props;
use crate::session::SaveSession;

/// Parsed `GlobalPalStorage.sav` plus everything `add_gps_pal`/
/// `add_gps_pal_from_dto`/`delete_gps_pals`/`rebuild_gps_save` need to read
/// and mutate its `SaveParameterArray` without re-parsing the file. Mirrors
/// `SaveManager._gps_gvas_file`/`_gps_pals` (`game/save_manager.py`).
#[derive(Default)]
pub struct GpsState {
    /// Temp file path recorded at load time (Steam sibling file, or an
    /// uploaded zip's staged copy -- see `psp-server/src/handlers/
    /// save_file.rs`'s `zip_gps_temp_path`), set before `save` is ever
    /// parsed. Mirrors Python's "path known, contents lazy" pattern
    /// (`AppState.gps_available`, `state.py:177-178`) -- `gps_available`
    /// below is true as soon as this is `Some`, even before `load_gps` runs.
    pub file_path: Option<PathBuf>,
    /// Parsed once `load_gps` runs; `None` until then.
    pub save: Option<uesave::Save>,
    /// `SaveParameterArray` length as of the last `load_gps`/`rebuild_gps_save`
    /// call -- every valid slot index is `0..slot_count`, occupied or not.
    pub slot_count: usize,
    /// index -> occupied slot, mirroring Python's `_gps_pals: Dict[int,
    /// Pal]`, but holding a `PalDto` snapshot (this port's DTO/functional
    /// architecture, not a live `Pal` handle) instead. Every mutator here
    /// re-derives the entries it touches from the just-written slot
    /// immediately, so this never goes stale between calls.
    pub pals: BTreeMap<i32, PalDto>,
    pub loaded: bool,
}

/// `GlobalPalStorage.sav`'s root-level `SaveParameterArray`, read-only.
fn gps_slots(save: &uesave::Save) -> Option<&Vec<StructValue>> {
    props::struct_values(
        save.root
            .properties
            .0
            .get(&PropertyKey::from("SaveParameterArray"))?,
    )
}

/// `GlobalPalStorage.sav`'s root-level `SaveParameterArray`, mutable.
fn gps_slots_mut(save: &mut uesave::Save) -> Option<&mut Vec<StructValue>> {
    props::struct_values_mut(
        save.root
            .properties
            .0
            .get_mut(&PropertyKey::from("SaveParameterArray"))?,
    )
}

/// Port of the emptiness check inlined in `_find_first_empty_gps_slot`
/// (`pal_ops.py:130-137`): a slot is empty when its `CharacterID` is absent
/// or the literal string `"None"`. Identical in shape (not by accident) to
/// `pal::first_empty_dps_slot`'s own per-slot predicate -- GPS and DPS slots
/// share the exact same `SaveParameter` layout.
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

/// Scans `slots` and rebuilds an index -> `PalDto` map of every occupied
/// entry, skipping (never erroring on) any slot `pal::pal_dto_from_dps_slot`
/// can't parse -- matching this port's established "malformed entry is
/// skipped, not fatal" policy for untrusted save data.
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
    /// Port of `AppState.gps_available` (`state.py:177-178`): true once a
    /// GPS path is known (even before parsing) or the file has actually been
    /// loaded.
    pub fn gps_available(&self) -> bool {
        self.gps.loaded || self.gps.file_path.is_some()
    }

    /// Port of `PalOpsMixin.get_gps` (`pal_ops.py:50-51`). `None` until
    /// `load_gps` has run at least once.
    pub fn gps_pals(&self) -> Option<&BTreeMap<i32, PalDto>> {
        self.gps.loaded.then_some(&self.gps.pals)
    }

    /// Port of `PalOpsMixin.load_gps`/`_load_gps_pals`
    /// (`pal_ops.py:33-48`): parses `GlobalPalStorage.sav` (with the same
    /// Palworld type hints every other save file in this port uses --
    /// `session::parse_palworld_save`, not a bare `uesave::Save::read`) and
    /// extracts every non-empty `SaveParameterArray` slot as a `PalDto`.
    pub fn load_gps(
        &mut self,
        sav_bytes: &[u8],
        game_data: &GameData,
    ) -> Result<&BTreeMap<i32, PalDto>, CoreError> {
        let save = crate::session::parse_palworld_save(sav_bytes)?;
        let slots = gps_slots(&save).ok_or_else(|| {
            CoreError::Parse("GlobalPalStorage SaveParameterArray missing".into())
        })?;
        self.gps.pals = collect_gps_pals(slots, game_data);
        self.gps.slot_count = slots.len();
        self.gps.save = Some(save);
        self.gps.loaded = true;
        Ok(&self.gps.pals)
    }

    /// Port of `PalOpsMixin._find_first_empty_gps_slot` (`pal_ops.py:123-143`).
    /// Deviation from Python: Python raises `ValueError("GPS Gvas file is not
    /// initialized.")` when no GPS file is loaded; this returns `None`
    /// instead, matching this method's required `Option<i32>` (not `Result`)
    /// signature -- callers that need the distinction (`add_gps_pal`/
    /// `add_gps_pal_from_dto`) check `gps.loaded` themselves first and raise
    /// the exact same message.
    pub fn find_first_empty_gps_slot(&self) -> Option<i32> {
        let slots = gps_slots(self.gps.save.as_ref()?)?;
        slots
            .iter()
            .position(gps_slot_is_empty)
            .map(|index| index as i32)
    }

    /// Port of `PalOpsMixin.add_gps_pal` (`pal_ops.py:145-181`) via
    /// `Pal(data=pal_data, dps=True).reset()` + the explicit setter sequence
    /// there. Returns `(pal, index)`, matching Python's `return pal, slot_idx`.
    ///
    /// Unlike `add_player_dps_pal`'s `Player.add_dps_pal` (`new_pal=True`),
    /// Python's `add_gps_pal` constructs its `Pal` WITHOUT `new_pal=True`, so
    /// `_set_max_stomach()` never runs here -- no special `FullStomach`
    /// handling is needed (`reset_dps_save_parameter` already leaves
    /// `FullStomach` exactly as `reset()` does: untouched, whatever the
    /// slot's previous occupant left behind).
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
            // `pal.instance_id = uuid.uuid4()` -- net effect only. `reset()`'s
            // own `self.instance_id = EMPTY_UUID` (game/pal.py:636) is
            // immediately overwritten by this very next Python statement, so
            // (matching `add_player_dps_pal`'s established precedent) it is
            // never separately modeled here.
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
            // `pal.owner_uid = EMPTY_UUID` after `reset()`: a genuine no-op in
            // Python too -- `reset_dps_save_parameter` already writes
            // `OwnerPlayerUId` to `EMPTY_UUID`, so this is skipped rather than
            // duplicated.
            save_parameter.insert("CharacterID", props::name_property(character_id));
            save_parameter.insert("NickName", props::str_property(nickname));
            save_parameter.insert("FilteredNickName", props::str_property(nickname));
            // `pal.storage_id = EMPTY_UUID`: PARITY-BUG-1 (see
            // `reset_dps_save_parameter`'s own doc comment) -- ContainerId is
            // never actually touched by this setter, so this is skipped too.
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
            // `pal.hp = pal.max_hp`.
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

    /// Port of `PalOpsMixin.add_gps_pal_from_dto` (`pal_ops.py:183-214`) --
    /// also `PalOpsMixin.clone_gps_pal`'s entire body (`pal_ops.py:266-267`:
    /// `return self.add_gps_pal_from_dto(pal)`), so this method covers both.
    /// Returns `(index, pal)`, matching Python's `return slot_idx, pal`.
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
        // `pal_dto.owner_uid = EMPTY_UUID` -- a concrete (present) nil guid,
        // NOT `None`: `apply_pal_dto` only skips `OwnerPlayerUId` entirely
        // when the DTO field is `None` (`if let Some(owner_uid) = ...`), so
        // this must be `Some(EMPTY_UUID)`, matching Python's literal
        // assignment, to actually clear ownership on the clone.
        incoming.owner_uid = Some(props::EMPTY_UUID);
        incoming.instance_id = new_instance_id;
        incoming.storage_id = props::EMPTY_UUID; // inert -- PARITY-BUG-1, kept for fidelity
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
            // `apply_pal_dto` reproduces `Pal.update_from` in full, including
            // its tail `self.hp = self.max_hp` and boss-prefix formatting --
            // no separate Hp write is needed here (unlike `clone_dps_pal`,
            // whose Python source genuinely does write Hp a second,
            // redundant time after `populate_status_point_lists()`;
            // `add_gps_pal_from_dto` does not).
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

    /// Port of `PalOpsMixin.delete_gps_pals` (`pal_ops.py:373-382`), descending
    /// index order (doesn't affect the outcome; reproduced for fidelity).
    /// Only resets a slot whose index is currently tracked in `gps.pals`
    /// (`if pal_idx in self._gps_pals`), matching Python's guard exactly.
    ///
    /// Also clears the slot's OUTER `InstanceId.InstanceId` field to
    /// `EMPTY_UUID`, matching `Pal.reset()`'s `self.instance_id =
    /// PalObjects.EMPTY_UUID` (`game/pal.py:636`) -- the same fix
    /// `delete_player_dps_pals` already applies for the DPS-array case (see
    /// its own doc comment); unlike `add_gps_pal`, nothing overwrites it
    /// again afterward here, so it must actually be written.
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

    /// Re-derives `gps.pals`/`gps.slot_count` from the live, already-mutated
    /// `GlobalPalStorage.sav` property tree. Every mutator above
    /// (`add_gps_pal`/`add_gps_pal_from_dto`/`delete_gps_pals`) already
    /// writes directly into `gps.save`'s `SaveParameterArray` in place and
    /// keeps `gps.pals` in sync as it goes -- unlike the brief's assumed
    /// "detached `Pal` objects, reassembled into a fresh array on save"
    /// design, there is no separate array to rebuild. This exists as the
    /// defensive resync/verification step the Phase-4 save-to-disk path can
    /// call before compressing `gps.save` back to bytes (mirroring
    /// `level_sav_bytes`/`player_sav_bytes`'s own "compress the current tree"
    /// shape), and to give test coverage a concrete, observable post-mutation
    /// check.
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

    /// Compresses the loaded `GlobalPalStorage.sav` tree back to its `.sav`
    /// bytes, or `None` when no GPS file was loaded. Same shape as
    /// `level_sav_bytes`/`level_meta_sav_bytes`/`player_sav_bytes` -- the
    /// Phase-4 save-to-disk path's natural counterpart for GPS.
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
    use uesave::{
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

    /// A minimal `SaveSession` with `gps` pre-loaded (bypassing
    /// `load_gps`/its palworld-typed parser, which only real compressed
    /// bytes exercise -- see the corpus-gated test at the bottom of this
    /// file) from a three-slot `SaveParameterArray`: slot 0 empty (template),
    /// slot 1 already holding a lucky pal, slot 2 empty -- two empty slots
    /// so a test that fills one (`add_gps_pal`) still has a second free slot
    /// left for a subsequent `add_gps_pal_from_dto` clone.
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

    /// Pins the same `reset()`-never-touches-`IsRarePal` quirk
    /// `add_player_dps_pal_into_a_recycled_slot_inherits_a_stale_is_rare_pal_flag`
    /// pins for DPS -- GPS shares the exact same `reset_dps_save_parameter`.
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
    fn add_gps_pal_errors_with_pythons_message_when_not_loaded() {
        let level = minimal_save(Properties::default());
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        let data = game_data();
        let error = session
            .add_gps_pal(&data, "SheepBall", "x", None)
            .unwrap_err();
        assert_eq!(error.to_string(), "GPS Gvas file is not initialized.");
    }

    // `gps_sav_bytes` re-serializing a REAL, `load_gps`-parsed tree is
    // exercised by the corpus-gated test at the bottom of this file, not
    // here: `uesave`'s writer needs a property's schema, recorded only when
    // that property was actually read through `uesave::SaveReader` --
    // `gps_fixture_session`'s hand-built `Save` (a bare `Properties`
    // literal, never parsed) has no schema for `SaveParameterArray` at all,
    // so `write_sav_bytes` on it fails with `missing property schema`
    // regardless of any GPS logic. This is a property of every synthetic,
    // never-parsed `uesave::Save` in this crate (`pal_crud.rs`'s own DPS
    // fixtures never attempt to re-serialize either) -- not a GPS-specific
    // gap, and not evidence that `rebuild_gps_save`/`add_gps_pal` need to
    // register new schemas themselves: every property they write already
    // existed on the ORIGINAL, actually-parsed slot (see this task's
    // report).

    #[test]
    fn gps_sav_bytes_is_none_when_not_loaded() {
        let level = minimal_save(Properties::default());
        let session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        assert!(session.gps_sav_bytes().unwrap().is_none());
    }

    // ========================================================================
    // Corpus-gated real-file round trip -- set PSP_TEST_GPS_SAV to a real
    // GlobalPalStorage.sav to exercise the actual `load_gps` parse path
    // (palworld type hints, real compression) end to end; skips cleanly
    // otherwise, matching this workspace's established corpus-test
    // convention (`pal_crud.rs`'s own final test, `session.rs`'s
    // `test_load_real_steam_save`).
    // ========================================================================

    #[test]
    fn gps_load_add_clone_delete_round_trips_against_a_real_file() {
        let Some(gps_path) = std::env::var_os("PSP_TEST_GPS_SAV") else {
            eprintln!("PSP_TEST_GPS_SAV not set, skipping");
            return;
        };
        let gps_bytes = std::fs::read(gps_path).expect("fixture readable");
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
