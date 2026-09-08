#include "command_common.hpp"
#include "game_call.hpp"
#include "game_commands.hpp"
#include "snapshots.hpp"

#include <Unreal/FField.hpp>

#include <cstddef>

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt::snap;

namespace
{
constexpr const char* kInventoryOp = "inventory";
constexpr const wchar_t* kInventoryTypeEnum = STR("/Script/Pal.EPalPlayerInventoryType");

struct ContainerKind
{
    const wchar_t* enumerator;
    const char* wire;
};

constexpr ContainerKind kContainerKinds[] = {
    {STR("Common"), "common"},
    {STR("Essential"), "essential"},
    {STR("WeaponLoadout"), "weaponLoadout"},
    {STR("PlayerEquipArmor"), "playerEquipArmor"},
    {STR("FoodEquip"), "foodEquip"},
};

struct ObjectReturn
{
    UObject* ReturnValue{};
};

struct TryGetContainerParams
{
    uint8_t inventoryType{};
    UObject* OutContainer{};
    bool ReturnValue{};
};

static_assert(offsetof(TryGetContainerParams, inventoryType) == 0);
static_assert(offsetof(TryGetContainerParams, OutContainer) == 8);
static_assert(offsetof(TryGetContainerParams, ReturnValue) == 16);

int32_t parm_count(UFunction* fn)
{
    int32_t count = 0;
    for (FProperty* prop : TFieldRange<FProperty>(fn, EFieldIterationFlags::None))
    {
        if (prop && prop->HasAnyPropertyFlags(CPF_Parm))
        {
            ++count;
        }
    }
    return count;
}

bool try_get_container_layout_matches(UFunction* fn)
{
    if (parm_count(fn) != 3)
    {
        return false;
    }

    FProperty* type_prop = amity_rt::find_struct_prop(fn, {STR("inventoryType")});
    if (!type_prop || type_prop->GetSize() != 1 ||
        type_prop->GetOffset_ForInternal() != static_cast<int32_t>(offsetof(TryGetContainerParams, inventoryType)))
    {
        return false;
    }
    if (!CastField<FEnumProperty>(type_prop) && !CastField<FByteProperty>(type_prop))
    {
        return false;
    }

    auto* out_prop = CastField<FObjectProperty>(amity_rt::find_struct_prop(fn, {STR("OutContainer")}));
    if (!out_prop || out_prop->GetSize() != static_cast<int32_t>(sizeof(UObject*)) ||
        out_prop->GetOffset_ForInternal() != static_cast<int32_t>(offsetof(TryGetContainerParams, OutContainer)))
    {
        return false;
    }
    UClass* declared = out_prop->GetPropertyClass().Get();
    UClass* container_class = amity_rt::find_class(STR("/Script/Pal.PalItemContainer"));
    if (!declared || !container_class || !declared->IsChildOf(container_class))
    {
        return false;
    }

    auto* return_prop = CastField<FBoolProperty>(amity_rt::find_struct_prop(fn, {STR("ReturnValue")}));
    return return_prop && return_prop->GetSize() == 1 &&
            return_prop->GetOffset_ForInternal() == static_cast<int32_t>(offsetof(TryGetContainerParams, ReturnValue));
}

struct DynamicItemClasses
{
    UClass* base{};
    UClass* weapon{};
    UClass* armor{};
    UClass* egg{};
};

DynamicItemClasses resolve_dynamic_item_classes()
{
    DynamicItemClasses classes{};
    classes.base = amity_rt::find_class(STR("/Script/Pal.PalDynamicItemDataBase"));
    classes.weapon = amity_rt::find_class(STR("/Script/Pal.PalDynamicWeaponItemDataBase"));
    classes.armor = amity_rt::find_class(STR("/Script/Pal.PalDynamicArmorItemDataBase"));
    classes.egg = amity_rt::find_class(STR("/Script/Pal.PalDynamicPalEggItemDataBase"));
    return classes;
}

// value_ptr reinterprets whatever address it is handed, so a leaf that is not really an FName
// would be ToString()'d against a bogus name-table index. The property class is the gate.
template <typename Container>
FName* name_value(FProperty* prop, Container* container)
{
    auto* name_prop = CastField<FNameProperty>(prop);
    return name_prop ? value_ptr<FName>(name_prop, container) : nullptr;
}

nlohmann::json guid_member(UObject* owner, const wchar_t* struct_member, const wchar_t* expected_struct, const wchar_t* guid_member_name)
{
    auto* struct_prop = CastField<FStructProperty>(amity_rt::find_prop(owner, {struct_member}));
    if (!struct_prop)
    {
        return nullptr;
    }
    UStruct* type = struct_prop->GetStruct();
    if (!type || type->GetName() != expected_struct)
    {
        return nullptr;
    }
    void* ptr = struct_prop->ContainerPtrToValuePtr<void>(owner);
    if (FGuid* guid = value_ptr<FGuid>(amity_rt::find_struct_prop(type, {guid_member_name}), ptr))
    {
        return format_guid(*guid);
    }
    return nullptr;
}

nlohmann::json read_dynamic_item(UObject* dynamic_data, const DynamicItemClasses& classes)
{
    if (!dynamic_data || !classes.base || !dynamic_data->IsA(classes.base))
    {
        return nullptr;
    }

    nlohmann::json entry = nlohmann::json::object();
    entry["characterId"] = nullptr;
    entry["durability"] = nullptr;
    entry["localId"] = guid_member(dynamic_data, STR("ID"), STR("PalDynamicItemId"), STR("LocalIdInCreatedWorld"));
    entry["maxDurability"] = nullptr;
    entry["passiveSkills"] = read_name_array(amity_rt::find_prop(dynamic_data, {STR("PassiveSkillList")}), dynamic_data);
    entry["remainingBullets"] = nullptr;
    entry["type"] = nullptr;

    if (classes.weapon && dynamic_data->IsA(classes.weapon))
    {
        entry["type"] = "weapon";
        if (auto bullets = read_numeric(amity_rt::find_prop(dynamic_data, {STR("RemainingBullets")}), dynamic_data))
        {
            entry["remainingBullets"] = *bullets;
        }
    }
    else if (classes.armor && dynamic_data->IsA(classes.armor))
    {
        entry["type"] = "armor";
    }
    else if (classes.egg && dynamic_data->IsA(classes.egg))
    {
        entry["type"] = "egg";
        if (FName* character_id = name_value(amity_rt::find_prop(dynamic_data, {STR("CharacterID")}), dynamic_data))
        {
            entry["characterId"] = to_utf8(character_id->ToString());
        }
    }

    if (auto durability = read_float(amity_rt::find_prop(dynamic_data, {STR("Durability")}), dynamic_data))
    {
        entry["durability"] = *durability;
    }
    if (auto max_durability = read_float(amity_rt::find_prop(dynamic_data, {STR("MaxDurability")}), dynamic_data))
    {
        entry["maxDurability"] = *max_durability;
    }
    return entry;
}

// DynamicItemData is a TWeakObjectPtr, so the raw bytes are an object index and serial number
// rather than a pointer. FObjectPropertyBase::GetObjectPropertyValue dispatches into the
// engine's own accessor for the concrete property class, which validates the serial number and
// yields null for a stale reference; nothing here reinterprets those bytes itself.
UObject* resolve_dynamic_item_data(UObject* slot)
{
    FProperty* prop = amity_rt::find_prop(slot, {STR("DynamicItemData")});
    if (!CastField<FWeakObjectProperty>(prop) && !CastField<FObjectProperty>(prop))
    {
        return nullptr;
    }
    auto* object_prop = CastField<FObjectPropertyBase>(prop);
    if (!object_prop)
    {
        return nullptr;
    }
    return object_prop->GetObjectPropertyValue(object_prop->ContainerPtrToValuePtr<void>(slot));
}

nlohmann::json read_slot(UObject* slot, int32_t array_index, const DynamicItemClasses& classes, bool& degraded)
{
    auto* item_id_prop = CastField<FStructProperty>(amity_rt::find_prop(slot, {STR("ItemId")}));
    UStruct* item_id_struct = item_id_prop ? item_id_prop->GetStruct() : nullptr;
    if (!item_id_struct || item_id_struct->GetName() != STR("PalItemId"))
    {
        degraded = true;
        return nullptr;
    }

    void* item_id_ptr = item_id_prop->ContainerPtrToValuePtr<void>(slot);
    FName* static_id = name_value(amity_rt::find_struct_prop(item_id_struct, {STR("StaticId")}), item_id_ptr);
    if (!static_id)
    {
        degraded = true;
        return nullptr;
    }
    if (static_id->IsNone())
    {
        return nullptr;
    }
    const std::string static_item_id = to_utf8(static_id->ToString());
    if (static_item_id.empty() || static_item_id == "None")
    {
        return nullptr;
    }

    nlohmann::json entry = nlohmann::json::object();
    entry["count"] = nullptr;
    entry["dynamicItem"] = read_dynamic_item(resolve_dynamic_item_data(slot), classes);
    entry["slotIndex"] = array_index;
    entry["staticItemId"] = static_item_id;

    if (auto count = read_numeric(amity_rt::find_prop(slot, {STR("StackCount")}), slot))
    {
        entry["count"] = *count;
    }
    else
    {
        degraded = true;
    }

    if (auto slot_index = read_numeric(amity_rt::find_prop(slot, {STR("SlotIndex")}), slot))
    {
        entry["slotIndex"] = static_cast<int32_t>(*slot_index);
    }
    else
    {
        degraded = true;
    }
    return entry;
}

// FArrayProperty::GetSize() is 16 for every TArray whatever it holds, so the array alone proves
// nothing about its elements. We walk each element as a UObject* item slot, so the inner has to
// be an object pointer of that stride whose declared class is a PalItemSlot.
FArrayProperty* item_slot_array(UObject* container)
{
    auto* array_prop = CastField<FArrayProperty>(amity_rt::find_prop(container, {STR("ItemSlotArray")}));
    if (!array_prop)
    {
        return nullptr;
    }
    auto* inner = CastField<FObjectProperty>(array_prop->GetInner());
    if (!inner || inner->GetSize() != static_cast<int32_t>(sizeof(UObject*)))
    {
        return nullptr;
    }
    UClass* element_class = inner->GetPropertyClass().Get();
    UClass* slot_class = amity_rt::find_class(STR("/Script/Pal.PalItemSlot"));
    if (!element_class || !slot_class || !element_class->IsChildOf(slot_class))
    {
        return nullptr;
    }
    return array_prop;
}

nlohmann::json read_container(UObject* container, const DynamicItemClasses& classes, bool& degraded)
{
    nlohmann::json entry = nlohmann::json::object();
    entry["containerId"] = guid_member(container, STR("ID"), STR("PalContainerId"), STR("ID"));
    entry["slotNum"] = nullptr;
    entry["slots"] = nlohmann::json::array();

    FArrayProperty* array_prop = item_slot_array(container);
    if (!array_prop)
    {
        degraded = true;
        entry["status"] = "partial";
        return entry;
    }

    FScriptArrayHelper helper(array_prop, array_prop->ContainerPtrToValuePtr<void>(container));
    entry["slotNum"] = helper.Num();

    auto* inner = CastField<FObjectProperty>(array_prop->GetInner());
    bool container_degraded = false;
    for (int32_t i = 0; i < helper.Num(); ++i)
    {
        uint8_t* element = helper.GetRawPtr(i);
        if (!element)
        {
            container_degraded = true;
            continue;
        }
        UObject* slot = inner->GetObjectPropertyValue(element);
        if (!slot)
        {
            continue;
        }
        nlohmann::json slot_entry = read_slot(slot, i, classes, container_degraded);
        if (!slot_entry.is_null())
        {
            entry["slots"].push_back(std::move(slot_entry));
        }
    }

    if (container_degraded)
    {
        degraded = true;
    }
    entry["status"] = container_degraded ? "partial" : "ok";
    return entry;
}
}

namespace amity_rt
{
amity::GameResponse snapshot_inventory(const nlohmann::json& args)
{
    FGuid player_uid{};
    std::string reason{};
    if (!amity_rt::parse_player_uid(args, player_uid, reason))
    {
        return amity::GameResponse::fail("validation_failed", reason);
    }

    if (!read_op_signature_ok(kInventoryOp, reason))
    {
        return amity::GameResponse::fail("capability_unavailable", reason);
    }

    UObject* world_context = any_player_controller();
    if (!world_context)
    {
        return amity::GameResponse::fail("capability_unavailable", "no live world context");
    }

    UFunction* get_inventory_fn = find_function(STR("/Script/Pal.PalPlayerState:GetInventoryData"));
    UFunction* try_get_container_fn = find_function(STR("/Script/Pal.PalPlayerInventoryData:TryGetContainerFromInventoryType"));
    if (!get_inventory_fn || !try_get_container_fn)
    {
        return amity::GameResponse::fail("capability_unavailable", "inventory functions unresolved");
    }
    if (!try_get_container_layout_matches(try_get_container_fn))
    {
        return amity::GameResponse::fail("capability_unavailable", "signature mismatch: TryGetContainerFromInventoryType");
    }
    if (!find_enum(kInventoryTypeEnum))
    {
        return amity::GameResponse::fail("capability_unavailable", "unresolved: EPalPlayerInventoryType");
    }

    UObject* player_state = player_state_by_uid(world_context, player_uid);
    if (!player_state)
    {
        return amity::GameResponse::fail("validation_failed", "unknown playerUid");
    }

    ObjectReturn inventory_params{};
    player_state->ProcessEvent(get_inventory_fn, &inventory_params);
    UObject* inventory = inventory_params.ReturnValue;
    if (!inventory)
    {
        return amity::GameResponse::fail("game_error", "inventory data unavailable");
    }

    const DynamicItemClasses classes = resolve_dynamic_item_classes();

    nlohmann::json containers = nlohmann::json::array();
    bool degraded = false;
    for (const ContainerKind& kind : kContainerKinds)
    {
        nlohmann::json entry = nlohmann::json::object();
        entry["containerId"] = nullptr;
        entry["slotNum"] = nullptr;
        entry["slots"] = nlohmann::json::array();
        entry["status"] = "unavailable";
        entry["type"] = kind.wire;

        std::optional<int64_t> ordinal = enum_value_by_name(kInventoryTypeEnum, kind.enumerator);
        if (!ordinal || *ordinal < 0 || *ordinal > 0xFF)
        {
            degraded = true;
            containers.push_back(std::move(entry));
            continue;
        }

        TryGetContainerParams container_params{};
        container_params.inventoryType = static_cast<uint8_t>(*ordinal);
        inventory->ProcessEvent(try_get_container_fn, &container_params);
        if (!container_params.ReturnValue || !container_params.OutContainer)
        {
            degraded = true;
            containers.push_back(std::move(entry));
            continue;
        }

        nlohmann::json read = read_container(container_params.OutContainer, classes, degraded);
        read["type"] = kind.wire;
        containers.push_back(std::move(read));
    }

    amity::GameResponse response;
    response.data = {
        {"containers", std::move(containers)},
        {"playerUid", format_guid(player_uid)},
        {"status", degraded ? "partial" : "ok"},
    };
    return response;
}
}

namespace
{
nlohmann::json usage_type_name(UObject* module)
{
    FProperty* prop = amity_rt::find_prop(module, {STR("UsageType")});
    auto* enum_prop = CastField<FEnumProperty>(prop);
    FNumericProperty* underlying = enum_prop ? enum_prop->GetUnderlyingProperty() : nullptr;
    UEnum* enum_type = enum_prop ? enum_prop->GetEnum() : nullptr;
    if (!underlying || !enum_type)
    {
        return nullptr;
    }
    void* value_ptr_raw = enum_prop->ContainerPtrToValuePtr<void>(module);
    const int64_t value = underlying->GetSignedIntPropertyValue(value_ptr_raw);
    return bare_enumerator(to_utf8(enum_type->GetNameByValue(value).ToString()));
}

}

namespace amity_rt
{
amity::GameResponse snapshot_guild_containers(const nlohmann::json& args)
{
    if (!args.contains("guildId") || !args["guildId"].is_string())
    {
        return amity::GameResponse::fail("validation_failed", "guildId is required");
    }
    const std::optional<FGuid> guild_id = parse_guid(args["guildId"].get<std::string>());
    if (!guild_id)
    {
        return amity::GameResponse::fail("validation_failed", "guildId is not a valid GUID");
    }

    UObject* world_context = any_player_controller();
    if (!world_context)
    {
        return amity::GameResponse::fail("capability_unavailable", "no live world context");
    }

    struct ObjectFacts
    {
        std::string build_object_id;
        std::string base_id;
    };
    std::map<std::string, ObjectFacts> facts_by_instance;
    {
        std::vector<UObject*> object_models{};
        UObjectGlobals::FindAllOf(STR("PalMapObjectModel"), object_models);
        for (UObject* object_model : object_models)
        {
            if (!object_model)
            {
                continue;
            }
            FGuid* group = value_ptr<FGuid>(amity_rt::find_prop(object_model, {STR("GroupIdBelongTo")}), object_model);
            if (!group || !(*group == *guild_id))
            {
                continue;
            }
            FGuid* instance = value_ptr<FGuid>(amity_rt::find_prop(object_model, {STR("InstanceId")}), object_model);
            if (!instance)
            {
                continue;
            }
            ObjectFacts facts{};
            if (FName* build_id = value_ptr<FName>(amity_rt::find_prop(object_model, {STR("BuildObjectId")}), object_model))
            {
                facts.build_object_id = to_utf8(build_id->ToString());
            }
            if (FGuid* base = value_ptr<FGuid>(amity_rt::find_prop(object_model, {STR("BaseCampIdBelongTo")}), object_model))
            {
                const FGuid empty{};
                if (!(*base == empty))
                {
                    facts.base_id = format_guid(*base);
                }
            }
            facts_by_instance.emplace(format_guid(*instance), std::move(facts));
        }
    }

    const DynamicItemClasses classes = resolve_dynamic_item_classes();
    nlohmann::json containers = nlohmann::json::array();
    bool degraded = false;

    if (UObject* guild = guild_by_id(*guild_id))
    {
        if (UObject* storage = snap::object_member(guild, STR("ItemStorage")))
        {
            if (UObject* chest = snap::object_member(storage, STR("ItemContainer")))
            {
                nlohmann::json entry = read_container(chest, classes, degraded);
                entry["type"] = "GuildChest";
                entry["buildObjectId"] = "GuildChest";
                entry["baseId"] = nullptr;
                containers.push_back(std::move(entry));
            }
        }
    }

    std::vector<UObject*> modules{};
    UObjectGlobals::FindAllOf(STR("PalMapObjectItemContainerModule"), modules);
    for (UObject* module : modules)
    {
        if (!module)
        {
            continue;
        }
        UObject* container = snap::object_member(module, STR("TargetContainer"));
        if (!container)
        {
            continue;
        }
        FGuid* container_id = nullptr;
        auto* id_prop = CastField<FStructProperty>(amity_rt::find_prop(container, {STR("ID")}));
        if (id_prop && id_prop->GetStruct())
        {
            container_id = value_ptr<FGuid>(amity_rt::find_struct_prop(id_prop->GetStruct(), {STR("ID")}),
                                            id_prop->ContainerPtrToValuePtr<void>(container));
        }
        if (!container_id)
        {
            degraded = true;
            continue;
        }

        auto* belong_prop = CastField<FStructProperty>(amity_rt::find_prop(container, {STR("BelongInfo")}));
        UStruct* belong_type = belong_prop ? belong_prop->GetStruct() : nullptr;
        void* belong = belong_prop ? belong_prop->ContainerPtrToValuePtr<void>(container) : nullptr;
        if (!belong_type || !belong)
        {
            degraded = true;
            continue;
        }
        FGuid* owner = value_ptr<FGuid>(
            amity_rt::find_struct_prop(belong_type,
                                       {STR("GroupId"), STR("GroupIdBelongTo"), STR("OwnerGroupId"),
                                        STR("GuildId"), STR("BelongGroupId")}),
            belong);
        if (!owner)
        {
            degraded = true;
            continue;
        }
        if (!(*owner == *guild_id))
        {
            continue;
        }

        nlohmann::json entry = read_container(container, classes, degraded);
        entry["buildObjectId"] = nullptr;
        entry["baseId"] = nullptr;
        if (UObject* concrete = module->GetOuterPrivate())
        {
            if (FGuid* model_id = value_ptr<FGuid>(amity_rt::find_prop(concrete, {STR("ModelInstanceId")}), concrete))
            {
                const auto found = facts_by_instance.find(format_guid(*model_id));
                if (found != facts_by_instance.end())
                {
                    if (!found->second.build_object_id.empty())
                    {
                        entry["buildObjectId"] = found->second.build_object_id;
                    }
                    if (!found->second.base_id.empty())
                    {
                        entry["baseId"] = found->second.base_id;
                    }
                }
            }
        }
        entry["type"] = usage_type_name(module);
        containers.push_back(std::move(entry));
    }

    amity::GameResponse response;
    response.data = {
        {"guildId", args["guildId"]},
        {"containers", std::move(containers)},
        {"status", degraded ? "partial" : "ok"},
    };
    return response;
}
}
