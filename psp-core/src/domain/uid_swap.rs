//! Cross-player UID swap: exchanges two players' UIDs everywhere they appear in
//! `Level.sav`, their own `.sav`/`_dps.sav` files, and the on-disk file names.
//!
//! Fog of war is NOT among them. Its mask lives in `LocalData.sav` (see
//! `crate::localdata`), which is per-machine and absent from a dedicated-server save, so
//! there is nothing to exchange.

use crate::domain::guild_tail;
use crate::domain::{player, world};
use crate::error::CoreError;
use crate::progress::ProgressSink;
use crate::props;
use crate::session::SaveSession;
use crate::transfer::{self, TransferError};
use crate::ue::games::palworld::{PalMapConcreteModelVariant, PalStruct};
use crate::ue::{Arch, FGuid, Properties, Property, PropertyKey, StructValue, ValueVec};
use uuid::Uuid;

/// `None` means `current` is neither uid, which every caller reads as "leave it alone".
fn swapped(current: Uuid, old: Uuid, new: Uuid) -> Option<Uuid> {
    if current == old {
        Some(new)
    } else if current == new {
        Some(old)
    } else {
        None
    }
}

fn swap_guid_field(field: &mut FGuid, old: Uuid, new: Uuid) {
    if let Some(next) = swapped(props::guid_to_uuid(field), old, new) {
        *field = props::uuid_to_guid(next);
    }
}

fn swap_uuid_property(properties: &mut Properties, key: &str, old: Uuid, new: Uuid) {
    let Some(current) = props::get(properties, &[key]).and_then(props::as_uuid) else {
        return;
    };
    if let Some(next) = swapped(current, old, new) {
        properties.insert(key, props::guid_property(next));
    }
}

/// A pal's owner and its capture-bonus history of previous owners -- the fields
/// `blueprint::capture::character_entry_player_uids` reads, minus the entry key, which
/// only a player entry populates. Shared by `Level.sav` pals and `_dps.sav` slots, whose
/// `SaveParameter` bags have the same shape.
fn swap_save_parameter_owner(save_parameter: &mut Properties, old: Uuid, new: Uuid) {
    swap_uuid_property(save_parameter, "OwnerPlayerUId", old, new);

    if let Some(Property::Array(ValueVec::Struct(values))) = save_parameter
        .0
        .get_mut(&PropertyKey::from("OldOwnerPlayerUIds"))
    {
        for value in values.iter_mut() {
            if let StructValue::Guid(guid) = value {
                swap_guid_field(guid, old, new);
            }
        }
    }
}

/// Every pal in `CharacterSaveParameterMap` changes hands. Player entries are included
/// deliberately: they carry no `OwnerPlayerUId`, so the pass is a no-op on them, and a
/// filter would be one more thing to keep in step with the game's schema.
fn swap_pal_ownership(level: &mut crate::ue::Save, old: Uuid, new: Uuid) -> Result<(), CoreError> {
    for entry in world::character_map_mut(level)?.iter_mut() {
        if let Some(save_parameter) = world::entry_save_parameter_mut(entry) {
            swap_save_parameter_owner(save_parameter, old, new);
        }
    }
    Ok(())
}

/// A pal box travels with the `.sav` that names its container id, so the slot owners
/// recorded inside the container have to travel with it.
fn swap_character_container_slot_owners(
    level: &mut crate::ue::Save,
    old: Uuid,
    new: Uuid,
) -> Result<(), CoreError> {
    for entry in world::character_container_map_mut(level)?.iter_mut() {
        let Some(value_props) = props::struct_props_mut(&mut entry.value) else {
            continue;
        };
        let Some(slots) =
            props::get_mut(value_props, &["Slots"]).and_then(props::struct_values_mut)
        else {
            continue;
        };
        for slot in slots.iter_mut() {
            let StructValue::Struct(slot_props) = slot else {
                continue;
            };
            if let Some(Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw)))) =
                slot_props.0.get_mut(&PropertyKey::from("RawData"))
            {
                swap_guid_field(&mut raw.player_uid, old, new);
            }
        }
    }
    Ok(())
}

/// The write-side mirror of `blueprint::capture::structure_concrete_player_uids`. Matched
/// exhaustively for the same reason that one is: a variant that gains a player uid must
/// fail to compile here rather than silently stop being swapped.
///
/// `ModuleMap`'s `PasswordLock.player_infos` is deliberately absent -- it records who has
/// entered a code, not who owns the object, and no fixture save carries one to test with.
fn concrete_player_uid_fields_mut(
    variant: &mut PalMapConcreteModelVariant<Arch>,
) -> Vec<&mut FGuid> {
    match variant {
        PalMapConcreteModelVariant::ItemChest(model) => vec![&mut model.private_lock_player_uid],
        PalMapConcreteModelVariant::ItemChestAffectCorruption(model) => {
            vec![&mut model.private_lock_player_uid]
        }
        PalMapConcreteModelVariant::ItemBooth(model) => {
            let mut fields = vec![&mut model.private_lock_player_uid];
            fields.extend(
                model
                    .trade_infos
                    .iter_mut()
                    .map(|trade| &mut trade.seller_player_uid),
            );
            fields
        }
        PalMapConcreteModelVariant::DeathDroppedCharacter(model) => {
            vec![&mut model.owner_player_uid]
        }
        PalMapConcreteModelVariant::DeathPenaltyStorage(model) => vec![&mut model.owner_player_uid],
        PalMapConcreteModelVariant::DropItem(model) => vec![&mut model.pickupable_player_uid],
        PalMapConcreteModelVariant::Signboard(model) => vec![&mut model.last_modified_player_uid],
        PalMapConcreteModelVariant::PalEgg(model) => vec![&mut model.pickupdable_player_uid],
        PalMapConcreteModelVariant::CharacterTeamMission(_)
        | PalMapConcreteModelVariant::FarmSkillFruits(_)
        | PalMapConcreteModelVariant::SupplyStorage(_)
        | PalMapConcreteModelVariant::EnergyStorage(_)
        | PalMapConcreteModelVariant::ConvertItem(_)
        | PalMapConcreteModelVariant::PickupItemOnLevel(_)
        | PalMapConcreteModelVariant::ItemDropOnDamag(_)
        | PalMapConcreteModelVariant::DefenseBulletLauncher(_)
        | PalMapConcreteModelVariant::GenerateEnergy(_)
        | PalMapConcreteModelVariant::FarmBlockV2(_)
        | PalMapConcreteModelVariant::FastTravelPoint(_)
        | PalMapConcreteModelVariant::ShippingItem(_)
        | PalMapConcreteModelVariant::ProductItem(_)
        | PalMapConcreteModelVariant::RecoverOtomo(_)
        | PalMapConcreteModelVariant::HatchingEgg(_)
        | PalMapConcreteModelVariant::TreasureBox(_)
        | PalMapConcreteModelVariant::BreedFarm(_)
        | PalMapConcreteModelVariant::Lamp(_)
        | PalMapConcreteModelVariant::Torch(_)
        | PalMapConcreteModelVariant::BaseCampPoint(_)
        | PalMapConcreteModelVariant::Unknown(_) => Vec::new(),
    }
}

/// Who built each structure, plus the per-variant ownership, lock and pickup uids.
fn swap_map_object_ownership(
    level: &mut crate::ue::Save,
    old: Uuid,
    new: Uuid,
) -> Result<(), CoreError> {
    let Some(values) = world::map_object_values_mut(level)? else {
        return Ok(());
    };
    for value in values.iter_mut() {
        let StructValue::Struct(object_props) = value else {
            continue;
        };
        if let Some(Property::Struct(StructValue::Game(PalStruct::MapModel(model)))) = object_props
            .0
            .get_mut(&PropertyKey::from("Model"))
            .and_then(props::struct_props_mut)
            .and_then(|model_props| model_props.0.get_mut(&PropertyKey::from("RawData")))
        {
            swap_guid_field(&mut model.build_player_uid, old, new);
        }

        if let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModel(raw)))) =
            object_props
                .0
                .get_mut(&PropertyKey::from("ConcreteModel"))
                .and_then(props::struct_props_mut)
                .and_then(|concrete| concrete.0.get_mut(&PropertyKey::from("RawData")))
        {
            for field in concrete_player_uid_fields_mut(&mut raw.model_data) {
                swap_guid_field(field, old, new);
            }
        }
    }
    Ok(())
}

/// Retargets both UIDs in every guild -- character handles (matched by character
/// instance id), the admin uid, member `player_uid` -- bidirectionally in one pass.
fn swap_guild_member_uids(
    level: &mut crate::ue::Save,
    old_uid: Uuid,
    new_uid: Uuid,
    old_instance_id: Uuid,
    new_instance_id: Uuid,
) -> Result<(), CoreError> {
    for entry in world::group_map_mut(level)?.iter_mut() {
        if guild_tail::entry_group_type(entry).as_deref() != Some("EPalGroupType::Guild") {
            continue;
        }
        let Some(group_data) = guild_tail::entry_group_data_mut(entry) else {
            continue;
        };

        for handle in group_data.individual_character_handle_ids.iter_mut() {
            let handle_instance_id = props::guid_to_uuid(&handle.instance_id);
            if handle_instance_id == old_instance_id {
                handle.guid = props::uuid_to_guid(new_uid);
            } else if handle_instance_id == new_instance_id {
                handle.guid = props::uuid_to_guid(old_uid);
            }
        }

        let Some(guild) = guild_tail::as_guild_mut(group_data) else {
            continue;
        };
        guild_tail::swap_player_uids(guild, old_uid, new_uid);
    }
    Ok(())
}

impl SaveSession {
    /// Runs after `swap_player_file_refs` has already traded the `_dps.sav` trees, so
    /// each tree is sitting under its new owner and its slots still name the old one.
    fn swap_dps_pal_ownership(&mut self, old_uid: Uuid, new_uid: Uuid) {
        for uid in [old_uid, new_uid] {
            let Some(dps) = self
                .loaded_players
                .get_mut(&uid)
                .and_then(|loaded| loaded.dps.as_mut())
            else {
                continue;
            };
            let Some(slots) = dps
                .root
                .properties
                .0
                .get_mut(&PropertyKey::from("SaveParameterArray"))
                .and_then(props::struct_values_mut)
            else {
                continue;
            };
            for slot in slots.iter_mut() {
                let StructValue::Struct(slot_props) = slot else {
                    continue;
                };
                if let Some(save_parameter) = slot_props
                    .0
                    .get_mut(&PropertyKey::from("SaveParameter"))
                    .and_then(props::struct_props_mut)
                {
                    swap_save_parameter_owner(save_parameter, old_uid, new_uid);
                }
            }
        }
    }

    /// `TransferError::Rejected` is a SOFT rejection: reported as an
    /// `{"error": ...}` payload on the normal response, not a WS error frame.
    pub fn swap_player_uids(
        &mut self,
        old_player_uid: Uuid,
        new_player_uid: Uuid,
        progress: &ProgressSink,
    ) -> Result<(), TransferError> {
        if old_player_uid == new_player_uid {
            return Err(TransferError::Rejected("Both players are the same.".into()));
        }

        progress("Validating players...");
        for uid in [old_player_uid, new_player_uid] {
            if !self.player_file_refs.contains_key(&uid) {
                return Err(TransferError::Rejected(format!("Player {uid} not found.")));
            }
            transfer::ensure_player_gvas_loaded(self, uid)?;
        }

        let old_instance_id = self
            .loaded_players
            .get(&old_player_uid)
            .and_then(|loaded| player::save_data_props(&loaded.sav).ok())
            .and_then(transfer::save_data_instance_id)
            .ok_or_else(|| {
                TransferError::Rejected("Source player SaveData is missing or invalid.".into())
            })?;
        let new_instance_id = self
            .loaded_players
            .get(&new_player_uid)
            .and_then(|loaded| player::save_data_props(&loaded.sav).ok())
            .and_then(transfer::save_data_instance_id)
            .ok_or_else(|| {
                TransferError::Rejected("Target player SaveData is missing or invalid.".into())
            })?;

        progress("Swapping player UIDs in save data...");
        self.swap_player_gvas_uids(old_player_uid, new_player_uid);

        progress("Swapping UIDs in character save parameter map...");
        for entry in world::character_map_mut(&mut self.level)?.iter_mut() {
            let Some(instance_id) = world::entry_instance_id(entry) else {
                continue;
            };
            if instance_id == old_instance_id {
                world::set_entry_player_uid(entry, new_player_uid);
            } else if instance_id == new_instance_id {
                world::set_entry_player_uid(entry, old_player_uid);
            }
        }

        progress("Swapping UIDs in guild data...");
        swap_guild_member_uids(
            &mut self.level,
            old_player_uid,
            new_player_uid,
            old_instance_id,
            new_instance_id,
        )?;

        progress("Swapping pal ownership...");
        swap_pal_ownership(&mut self.level, old_player_uid, new_player_uid)?;
        swap_character_container_slot_owners(&mut self.level, old_player_uid, new_player_uid)?;

        progress("Swapping structure ownership...");
        swap_map_object_ownership(&mut self.level, old_player_uid, new_player_uid)?;

        progress("Swapping player file references...");
        self.swap_player_file_refs(old_player_uid, new_player_uid);
        self.swap_dps_pal_ownership(old_player_uid, new_player_uid);

        progress("Rebuilding caches...");
        self.rebuild_player_caches()?;

        Ok(())
    }
}
