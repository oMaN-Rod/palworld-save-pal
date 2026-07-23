//! Zeroes every player-identifying UID a captured `BaseBlueprint` carries.
//! `capture::capture` always runs this; nothing outside this module should
//! need to call it directly except tests exercising the precondition via
//! `capture::capture_unscrubbed`.

use uuid::Uuid;

use super::BaseBlueprint;
use crate::domain::world;
use crate::props;
use crate::ue::games::palworld::{PalMapConcreteModelModuleData, PalMapConcreteModelVariant};
use crate::ue::{FGuid, MapEntry, PalStruct, Properties, Property, PropertyKey, StructValue, ValueVec};

fn zero() -> FGuid {
    props::uuid_to_guid(Uuid::nil())
}

fn scrub_model(properties: &mut Properties) {
    let Some(model) = properties
        .0
        .get_mut(&PropertyKey::from("Model"))
        .and_then(props::struct_props_mut)
    else {
        return;
    };
    if let Some(Property::Struct(StructValue::Game(PalStruct::MapModel(raw)))) =
        model.0.get_mut(&PropertyKey::from("RawData"))
    {
        raw.build_player_uid = zero();
        raw.group_id_belong_to = zero();
    }
}

fn scrub_concrete_variant(variant: &mut PalMapConcreteModelVariant<crate::ue::Arch>) {
    match variant {
        PalMapConcreteModelVariant::ItemChest(model) => {
            model.private_lock_player_uid = zero();
        }
        PalMapConcreteModelVariant::ItemChestAffectCorruption(model) => {
            model.private_lock_player_uid = zero();
        }
        PalMapConcreteModelVariant::ItemBooth(model) => {
            model.private_lock_player_uid = zero();
            for trade in &mut model.trade_infos {
                trade.seller_player_uid = zero();
            }
        }
        PalMapConcreteModelVariant::DeathDroppedCharacter(model) => {
            model.owner_player_uid = zero();
        }
        PalMapConcreteModelVariant::DeathPenaltyStorage(model) => {
            model.owner_player_uid = zero();
        }
        PalMapConcreteModelVariant::DropItem(model) => {
            model.pickupable_player_uid = zero();
        }
        PalMapConcreteModelVariant::Signboard(model) => {
            model.last_modified_player_uid = zero();
        }
        PalMapConcreteModelVariant::PalEgg(model) => {
            model.pickupdable_player_uid = zero();
        }
        // Exhaustive by design, not a style choice: a future uesave upgrade
        // that adds a variant here must fail to compile until someone
        // decides whether it carries a player UID, rather than silently
        // falling through a catch-all and leaking one.
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
        | PalMapConcreteModelVariant::Unknown(_) => {}
    }
}

/// Zeroes `player_uid` on every `PalPlayerLockInfo` inside a `PasswordLock`
/// module. Runs unconditionally, regardless of what `CaptureOptions` asked
/// for: `capture::clear_access_config` also drops the whole lock when the
/// user did not request access config, but this is the backstop that must
/// hold on every preset, including ones that keep the lock.
fn scrub_module_map(properties: &mut Properties) {
    let Some(concrete) = properties
        .0
        .get_mut(&PropertyKey::from("ConcreteModel"))
        .and_then(props::struct_props_mut)
    else {
        return;
    };
    let Some(module_entries) = concrete
        .0
        .get_mut(&PropertyKey::from("ModuleMap"))
        .and_then(props::map_entries_mut)
    else {
        return;
    };
    for module in module_entries {
        let Some(module_props) = props::struct_props_mut(&mut module.value) else {
            continue;
        };
        if let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModelModule(raw)))) =
            module_props.0.get_mut(&PropertyKey::from("RawData"))
        {
            if let PalMapConcreteModelModuleData::PasswordLock { player_infos, .. } = &mut raw.data {
                for info in player_infos {
                    info.player_uid = zero();
                }
            }
        }
    }
}

fn scrub_concrete(properties: &mut Properties) {
    let Some(concrete) = properties
        .0
        .get_mut(&PropertyKey::from("ConcreteModel"))
        .and_then(props::struct_props_mut)
    else {
        return;
    };
    if let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModel(raw)))) =
        concrete.0.get_mut(&PropertyKey::from("RawData"))
    {
        scrub_concrete_variant(&mut raw.model_data);
    }
}

/// Zeroes every slot's `player_uid` in a `CharacterContainerSaveData` entry.
fn scrub_character_container_entry(entry: &mut MapEntry) {
    let Some(value_props) = props::struct_props_mut(&mut entry.value) else {
        return;
    };
    let Some(slots) = props::get_mut(value_props, &["Slots"]).and_then(props::struct_values_mut)
    else {
        return;
    };
    for slot in slots {
        let StructValue::Struct(slot_props) = slot else {
            continue;
        };
        if let Some(Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw)))) =
            slot_props.0.get_mut(&PropertyKey::from("RawData"))
        {
            raw.player_uid = zero();
        }
    }
}

/// Zeroes a `CharacterSaveParameterMap` entry's key `PlayerUId`, the
/// `SaveParameter` bag's `OwnerPlayerUId`, and clears `OldOwnerPlayerUIds`.
fn scrub_character_entry(entry: &mut MapEntry) {
    world::set_entry_player_uid(entry, Uuid::nil());
    let Some(save_parameter) = world::entry_save_parameter_mut(entry) else {
        return;
    };
    save_parameter.insert("OwnerPlayerUId", props::guid_property(Uuid::nil()));
    if let Some(Property::Array(ValueVec::Struct(values))) =
        save_parameter.0.get_mut(&PropertyKey::from("OldOwnerPlayerUIds"))
    {
        values.clear();
    }
}

/// Zeroes `group_id_belong_to` on a captured `BaseCampSaveData` entry's typed
/// `PalStruct::BaseCamp` raw data.
fn scrub_base_camp(base_camp: &mut Properties) {
    if let Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) =
        base_camp.0.get_mut(&PropertyKey::from("RawData"))
    {
        raw.group_id_belong_to = zero();
    }
}

pub fn scrub_blueprint(blueprint: &mut BaseBlueprint) {
    for structure in &mut blueprint.structures {
        scrub_model(&mut structure.properties);
        scrub_concrete(&mut structure.properties);
        scrub_module_map(&mut structure.properties);
    }
    for entry in &mut blueprint.character_containers {
        scrub_character_container_entry(entry);
    }
    for entry in &mut blueprint.characters {
        scrub_character_entry(entry);
    }
    if let Some(base_camp) = &mut blueprint.base_camp {
        scrub_base_camp(base_camp);
    }
}
