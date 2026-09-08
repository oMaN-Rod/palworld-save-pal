#include "snapshots.hpp"

#include "command_common.hpp"
#include "game_call.hpp"
#include "game_commands.hpp"

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt::snap;

namespace
{
constexpr int32_t kPageSize = 30;
constexpr int64_t kPhysicalHealthSevere = 2;
constexpr int32_t kPartySize = 5;

struct GetStorageParams
{
    UObject* WorldContextObject{};
    FGuid PlayerUId{};
    UObject* ReturnValue{};
};

struct NoParamsIntReturn
{
    int32_t ReturnValue{};
};

struct GetSlotsInPageParams
{
    int32_t pageIndex{};
    TArray<UObject*> Slots{};
};

struct GetSlotBySlotIndexParams
{
    int32_t SlotIndex{};
    UObject* ReturnValue{};
};

struct NoParamsObjectReturn
{
    UObject* ReturnValue{};
};

// FPalCharacterSlotId is { FPalContainerId { FGuid ID }; int32 SlotIndex; }, returned
// by value. The signature table pins the layout this mirrors.
struct GetSlotIdParams
{
    FGuid ContainerId{};
    int32 SlotIndex{};
};

struct GetOtomoIndividualHandleParams
{
    int32_t SlotIndex{};
    UObject* ReturnValue{};
};

struct RequiredFunctions
{
    UFunction* get_storage{};
    UFunction* get_page_num{};
    UFunction* get_slots_in_page{};
    UFunction* get_slot_by_index{};
    UFunction* get_handle{};
    UFunction* try_get_individual_parameter{};
    UFunction* get_slot_index{};
    UFunction* get_slot_id{};
    UFunction* get_otomo_handle{};
};

RequiredFunctions resolve_functions()
{
    RequiredFunctions fns;
    fns.get_storage = amity_rt::find_function(STR("/Script/Pal.PalUtility:GetPalStorageDataByPlayerUID"));
    fns.get_page_num = amity_rt::find_function(STR("/Script/Pal.PalPlayerDataPalStorage:GetPageNum"));
    fns.get_slots_in_page = amity_rt::find_function(STR("/Script/Pal.PalPlayerDataPalStorage:GetSlotsInPage"));
    fns.get_slot_by_index = amity_rt::find_function(STR("/Script/Pal.PalPlayerDataPalStorage:GetSlotBySlotIndex"));
    fns.get_handle = amity_rt::find_function(STR("/Script/Pal.PalIndividualCharacterSlot:GetHandle"));
    fns.try_get_individual_parameter = amity_rt::find_function(STR("/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter"));
    fns.get_slot_index = amity_rt::find_function(STR("/Script/Pal.PalIndividualCharacterSlot:GetSlotIndex"));
    fns.get_slot_id = amity_rt::find_function(STR("/Script/Pal.PalIndividualCharacterSlot:GetSlotId"));
    fns.get_otomo_handle = amity_rt::find_function(STR("/Script/Pal.PalOtomoHolderComponentBase:GetOtomoIndividualHandle"));
    return fns;
}

void add_pal_condition_fields(nlohmann::json& entry, void* sp, UStruct* sp_struct)
{
    entry["isSick"] = nullptr;
    entry["isFainted"] = nullptr;
    entry["isAwakened"] = nullptr;
    entry["hp"] = nullptr;
    entry["maxHp"] = nullptr;

    if (auto sick = read_numeric(amity_rt::find_struct_prop(sp_struct, {STR("WorkerSick")}), sp))
    {
        entry["isSick"] = *sick != 0;
    }

    const auto physical = read_numeric(amity_rt::find_struct_prop(sp_struct, {STR("PhysicalHealth")}), sp);
    const auto revive_timer = read_float(amity_rt::find_struct_prop(sp_struct, {STR("PalReviveTimer")}), sp);
    if (physical || revive_timer)
    {
        entry["isFainted"] = (physical && *physical >= kPhysicalHealthSevere) ||
                             (revive_timer && *revive_timer > 0.0);
    }

    if (auto awakened = read_bool(amity_rt::find_struct_prop(sp_struct, {STR("bIsAwakening"), STR("IsAwakening")}), sp))
    {
        entry["isAwakened"] = *awakened;
    }
    if (auto hp = read_fixed_point(amity_rt::find_struct_prop(sp_struct, {STR("Hp"), STR("HP")}), sp))
    {
        entry["hp"] = *hp;
    }
    if (auto max_hp = read_fixed_point(amity_rt::find_struct_prop(sp_struct, {STR("MaxHP"), STR("MaxHp")}), sp))
    {
        entry["maxHp"] = *max_hp;
    }
}

nlohmann::json summarize_pal(UObject* handle, UObject* individual_parameter, int32_t slot_index)
{
    nlohmann::json entry = nlohmann::json::object();

    entry["instanceId"] = nullptr;
    entry["playerUid"] = nullptr;
    if (handle)
    {
        FProperty* id_prop = amity_rt::find_prop(handle, {STR("ID")});
        if (auto* id_struct_prop = CastField<FStructProperty>(id_prop))
        {
            void* id_ptr = id_struct_prop->ContainerPtrToValuePtr<void>(handle);
            UStruct* id_struct = id_struct_prop->GetStruct();
            if (FGuid* instance_id = value_ptr<FGuid>(amity_rt::find_struct_prop(id_struct, {STR("InstanceId")}), id_ptr))
            {
                entry["instanceId"] = format_guid(*instance_id);
            }
            if (FGuid* player_uid = value_ptr<FGuid>(amity_rt::find_struct_prop(id_struct, {STR("PlayerUId")}), id_ptr))
            {
                entry["playerUid"] = format_guid(*player_uid);
            }
        }
    }

    entry["characterId"] = nullptr;
    entry["nickname"] = nullptr;
    entry["level"] = nullptr;
    entry["gender"] = nullptr;
    entry["isLucky"] = nullptr;
    entry["ownerUid"] = nullptr;

    if (individual_parameter)
    {
        FProperty* save_parameter_prop = amity_rt::find_prop(individual_parameter, {STR("SaveParameter")});
        if (auto* save_parameter_struct_prop = CastField<FStructProperty>(save_parameter_prop))
        {
            void* sp = save_parameter_struct_prop->ContainerPtrToValuePtr<void>(individual_parameter);
            UStruct* sp_struct = save_parameter_struct_prop->GetStruct();

            if (FName* character_id = value_ptr<FName>(amity_rt::find_struct_prop(sp_struct, {STR("CharacterID")}), sp))
            {
                entry["characterId"] = to_utf8(character_id->ToString());
            }
            if (FString* nickname = value_ptr<FString>(amity_rt::find_struct_prop(sp_struct, {STR("NickName")}), sp))
            {
                entry["nickname"] = to_utf8(nickname->operator*());
            }
            if (uint8_t* level = value_ptr<uint8_t>(amity_rt::find_struct_prop(sp_struct, {STR("Level")}), sp))
            {
                entry["level"] = static_cast<int>(*level);
            }
            if (auto gender_value = read_numeric(amity_rt::find_struct_prop(sp_struct, {STR("Gender")}), sp))
            {
                if (auto name = gender_name(*gender_value))
                {
                    entry["gender"] = *name;
                }
            }
            if (auto is_rare = read_bool(amity_rt::find_struct_prop(sp_struct, {STR("IsRarePal")}), sp))
            {
                entry["isLucky"] = *is_rare;
            }
            if (FGuid* owner = value_ptr<FGuid>(amity_rt::find_struct_prop(sp_struct, {STR("OwnerPlayerUId")}), sp))
            {
                entry["ownerUid"] = format_guid(*owner);
            }
            add_pal_condition_fields(entry, sp, sp_struct);
        }
    }

    entry["slotIndex"] = slot_index;
    return entry;
}

nlohmann::json read_party(UObject* world_context,
                          const FGuid& player_uid,
                          const RequiredFunctions& fns,
                          bool& reachable)
{
    reachable = false;
    nlohmann::json party = nlohmann::json::array();

    std::string reason{};
    if (!amity_rt::read_op_signature_ok("party", reason) || !fns.get_otomo_handle || !fns.try_get_individual_parameter)
    {
        return party;
    }

    UObject* holder = amity_rt::otomo_holder(amity_rt::player_state_by_uid(world_context, player_uid));
    if (!holder)
    {
        return party;
    }

    for (int32_t i = 0; i < kPartySize; ++i)
    {
        GetOtomoIndividualHandleParams handle_params{};
        handle_params.SlotIndex = i;
        holder->ProcessEvent(fns.get_otomo_handle, &handle_params);
        UObject* handle = handle_params.ReturnValue;
        if (!handle)
        {
            continue;
        }

        NoParamsObjectReturn param_params{};
        handle->ProcessEvent(fns.try_get_individual_parameter, &param_params);
        party.push_back(summarize_pal(handle, param_params.ReturnValue, i));
    }

    reachable = true;
    return party;
}

void add_pal_detail_fields(nlohmann::json& entry, UObject* individual_parameter)
{
    entry["exp"] = nullptr;
    entry["rank"] = nullptr;
    entry["rankHp"] = nullptr;
    entry["rankAttack"] = nullptr;
    entry["rankDefense"] = nullptr;
    entry["rankCraftSpeed"] = nullptr;
    entry["talentHp"] = nullptr;
    entry["talentShot"] = nullptr;
    entry["talentDefense"] = nullptr;
    entry["passiveSkills"] = nlohmann::json::array();
    entry["equipWaza"] = nlohmann::json::array();
    entry["masteredWaza"] = nlohmann::json::array();
    entry["workSuitability"] = nlohmann::json::object();
    entry["sanity"] = nullptr;
    entry["stomach"] = nullptr;
    entry["maxStomach"] = nullptr;
    entry["friendshipPoint"] = nullptr;

    if (!individual_parameter)
    {
        return;
    }
    FProperty* save_parameter_prop = amity_rt::find_prop(individual_parameter, {STR("SaveParameter")});
    auto* save_parameter_struct_prop = CastField<FStructProperty>(save_parameter_prop);
    if (!save_parameter_struct_prop)
    {
        return;
    }
    void* sp = save_parameter_struct_prop->ContainerPtrToValuePtr<void>(individual_parameter);
    UStruct* sp_struct = save_parameter_struct_prop->GetStruct();

    if (int64_t* exp = value_ptr<int64_t>(amity_rt::find_struct_prop(sp_struct, {STR("Exp")}), sp))
    {
        entry["exp"] = *exp;
    }
    if (uint8_t* rank = value_ptr<uint8_t>(amity_rt::find_struct_prop(sp_struct, {STR("Rank")}), sp))
    {
        entry["rank"] = static_cast<int>(*rank);
    }
    if (uint8_t* rank_hp = value_ptr<uint8_t>(amity_rt::find_struct_prop(sp_struct, {STR("Rank_HP")}), sp))
    {
        entry["rankHp"] = static_cast<int>(*rank_hp);
    }
    if (uint8_t* rank_attack = value_ptr<uint8_t>(amity_rt::find_struct_prop(sp_struct, {STR("Rank_Attack")}), sp))
    {
        entry["rankAttack"] = static_cast<int>(*rank_attack);
    }
    if (uint8_t* rank_defense =
            value_ptr<uint8_t>(amity_rt::find_struct_prop(sp_struct, {STR("Rank_Defence"), STR("Rank_Defense")}), sp))
    {
        entry["rankDefense"] = static_cast<int>(*rank_defense);
    }
    if (uint8_t* rank_craft = value_ptr<uint8_t>(amity_rt::find_struct_prop(sp_struct, {STR("Rank_CraftSpeed")}), sp))
    {
        entry["rankCraftSpeed"] = static_cast<int>(*rank_craft);
    }
    if (uint8_t* talent_hp = value_ptr<uint8_t>(amity_rt::find_struct_prop(sp_struct, {STR("Talent_HP")}), sp))
    {
        entry["talentHp"] = static_cast<int>(*talent_hp);
    }
    if (uint8_t* talent_shot = value_ptr<uint8_t>(amity_rt::find_struct_prop(sp_struct, {STR("Talent_Shot")}), sp))
    {
        entry["talentShot"] = static_cast<int>(*talent_shot);
    }
    if (uint8_t* talent_defense = value_ptr<uint8_t>(amity_rt::find_struct_prop(sp_struct, {STR("Talent_Defense")}), sp))
    {
        entry["talentDefense"] = static_cast<int>(*talent_defense);
    }

    entry["passiveSkills"] = read_name_array(amity_rt::find_struct_prop(sp_struct, {STR("PassiveSkillList")}), sp);
    entry["equipWaza"] = read_enum_array(amity_rt::find_struct_prop(sp_struct, {STR("EquipWaza")}), sp);
    entry["masteredWaza"] = read_enum_array(amity_rt::find_struct_prop(sp_struct, {STR("MasteredWaza")}), sp);

    entry["workSuitability"] =
        read_work_suitability(amity_rt::find_struct_prop(sp_struct, {STR("GotWorkSuitabilityAddRankList")}), sp);

    if (auto sanity = read_float(amity_rt::find_struct_prop(sp_struct, {STR("SanityValue")}), sp))
    {
        entry["sanity"] = *sanity;
    }
    if (auto stomach = read_float(amity_rt::find_struct_prop(sp_struct, {STR("FullStomach")}), sp))
    {
        entry["stomach"] = *stomach;
    }
    if (auto max_stomach = read_float(amity_rt::find_struct_prop(sp_struct, {STR("MaxFullStomach")}), sp))
    {
        entry["maxStomach"] = *max_stomach;
    }
    if (auto friendship = read_numeric(amity_rt::find_struct_prop(sp_struct, {STR("FriendshipPoint")}), sp))
    {
        entry["friendshipPoint"] = *friendship;
    }
}

bool parse_common_args(const nlohmann::json& args, FGuid& player_uid, int32_t& page, std::string& error)
{
    if (!amity_rt::parse_player_uid(args, player_uid, error))
    {
        return false;
    }

    page = 0;
    if (args.contains("page"))
    {
        if (!args["page"].is_number_integer() || args["page"].get<int64_t>() < 0)
        {
            error = "page must be a non-negative integer";
            return false;
        }
        page = static_cast<int32_t>(args["page"].get<int64_t>());
    }
    return true;
}

UObject* resolve_storage_and_page_count(UObject* world_context,
                                         const RequiredFunctions& fns,
                                         const FGuid& player_uid,
                                         int32_t page,
                                         int32_t& page_count,
                                         std::string& error_code,
                                         std::string& error_message)
{
    if (!fns.get_storage || !fns.get_page_num)
    {
        error_code = "capability_unavailable";
        error_message = "pal storage functions unresolved";
        return nullptr;
    }

    GetStorageParams storage_params{};
    storage_params.WorldContextObject = world_context;
    storage_params.PlayerUId = player_uid;
    world_context->ProcessEvent(fns.get_storage, &storage_params);
    UObject* storage = storage_params.ReturnValue;
    if (!storage)
    {
        error_code = "validation_failed";
        error_message = "player not found";
        return nullptr;
    }

    NoParamsIntReturn page_num_params{};
    storage->ProcessEvent(fns.get_page_num, &page_num_params);
    page_count = page_num_params.ReturnValue;

    const bool page_in_range = (page_count > 0 && page < page_count) || (page_count == 0 && page == 0);
    if (!page_in_range)
    {
        error_code = "validation_failed";
        error_message = "page out of range";
        return nullptr;
    }
    return storage;
}
}

namespace amity_rt
{
amity::GameResponse snapshot_pals(const nlohmann::json& args)
{
    FGuid player_uid;
    int32_t page = 0;
    std::string error;
    if (!parse_common_args(args, player_uid, page, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }

    UObject* world_context = any_player_controller();
    if (!world_context)
    {
        return amity::GameResponse::fail("capability_unavailable", "no live world context");
    }

    RequiredFunctions fns = resolve_functions();
    int32_t page_count = 0;
    std::string error_code, error_message;
    UObject* storage = resolve_storage_and_page_count(world_context, fns, player_uid, page, page_count, error_code, error_message);
    if (!storage)
    {
        return amity::GameResponse::fail(error_code, error_message);
    }

    if (!fns.get_slots_in_page || !fns.get_handle || !fns.try_get_individual_parameter)
    {
        return amity::GameResponse::fail("capability_unavailable", "pal slot functions unresolved");
    }

    GetSlotsInPageParams slots_params{};
    slots_params.pageIndex = page;
    storage->ProcessEvent(fns.get_slots_in_page, &slots_params);

    nlohmann::json pals = nlohmann::json::array();
    bool list_partial = false;

    const int32_t slot_count = slots_params.Slots.Num();
    const int32_t page_slot_count = slot_count < kPageSize ? slot_count : kPageSize;

    int32_t slot_base = page * page_slot_count;
    nlohmann::json container_id = nullptr;
    std::string slot_id_reason{};
    if (page_slot_count > 0 && fns.get_slot_id && amity_rt::read_op_signature_ok("palSlot", slot_id_reason))
    {
        if (UObject* first_slot = slots_params.Slots[0])
        {
            GetSlotIdParams slot_id_params{};
            first_slot->ProcessEvent(fns.get_slot_id, &slot_id_params);
            slot_base = slot_id_params.SlotIndex;
            container_id = format_guid(slot_id_params.ContainerId);
        }
    }
    for (int32_t i = 0; i < page_slot_count; ++i)
    {
        UObject* slot = slots_params.Slots[i];
        if (!slot)
        {
            continue;
        }

        NoParamsObjectReturn handle_params{};
        slot->ProcessEvent(fns.get_handle, &handle_params);
        UObject* handle = handle_params.ReturnValue;
        if (!handle)
        {
            continue;
        }

        int32_t slot_index = i;
        if (fns.get_slot_index)
        {
            NoParamsIntReturn slot_index_params{};
            slot->ProcessEvent(fns.get_slot_index, &slot_index_params);
            slot_index = slot_index_params.ReturnValue;
        }

        NoParamsObjectReturn param_params{};
        handle->ProcessEvent(fns.try_get_individual_parameter, &param_params);
        UObject* individual_parameter = param_params.ReturnValue;
        if (!individual_parameter)
        {
            list_partial = true;
        }

        pals.push_back(summarize_pal(handle, individual_parameter, slot_index));
    }

    bool party_reachable = false;
    nlohmann::json party = read_party(world_context, player_uid, fns, party_reachable);

    amity::GameResponse response;
    response.data = {{"pals", pals},
                     {"page", page},
                     {"pageCount", page_count},
                     {"slotCount", page_slot_count},
                     {"slotBase", slot_base},
                     {"containerId", container_id},
                     {"party", party},
                     {"partyStatus", party_reachable ? "ok" : "unavailable"},
                     {"status", list_partial ? "partial" : "ok"}};
    return response;
}

amity::GameResponse snapshot_base_pals(const nlohmann::json& args)
{
    if (!args.contains("baseId") || !args["baseId"].is_string())
    {
        return amity::GameResponse::fail("validation_failed", "baseId is required");
    }
    const std::optional<FGuid> base_id = parse_guid(args["baseId"].get<std::string>());
    if (!base_id)
    {
        return amity::GameResponse::fail("validation_failed", "baseId is not a valid GUID");
    }

    UFunction* try_get_parameter =
        amity_rt::find_function(STR("/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter"));
    if (!try_get_parameter)
    {
        return amity::GameResponse::fail("capability_unavailable", "pal parameter function unresolved");
    }

    const char* container_reason = "";
    UObject* container = amity_rt::base_camp_container(*base_id, container_reason);
    if (!container)
    {
        return amity::GameResponse::fail("validation_failed", container_reason);
    }

    auto* slot_array = CastField<FArrayProperty>(amity_rt::find_prop(container, {STR("SlotArray")}));
    auto* slot_inner = slot_array ? CastField<FObjectProperty>(slot_array->GetInner()) : nullptr;
    if (!slot_inner || slot_inner->GetSize() != static_cast<int32_t>(sizeof(UObject*)))
    {
        return amity::GameResponse::fail("game_error", "base camp slots unreadable");
    }

    nlohmann::json pals = nlohmann::json::array();
    bool partial = false;
    FScriptArrayHelper helper(slot_array, slot_array->ContainerPtrToValuePtr<void>(container));
    for (int32_t i = 0; i < helper.Num(); ++i)
    {
        uint8_t* element = helper.GetRawPtr(i);
        UObject* slot = element ? slot_inner->GetObjectPropertyValue(element) : nullptr;
        if (!slot)
        {
            continue;
        }

        UObject* handle = object_member(slot, STR("Handle"));
        if (!handle)
        {
            continue;
        }

        int32_t slot_index = i;
        if (auto index = read_numeric(amity_rt::find_prop(slot, {STR("SlotIndex")}), slot))
        {
            slot_index = static_cast<int32_t>(*index);
        }

        NoParamsObjectReturn param_params{};
        handle->ProcessEvent(try_get_parameter, &param_params);
        if (!param_params.ReturnValue)
        {
            partial = true;
            continue;
        }
        pals.push_back(summarize_pal(handle, param_params.ReturnValue, slot_index));
    }

    amity::GameResponse response;
    response.data = {
        {"baseId", args["baseId"]},
        {"containerId", nested_guid(container, STR("ID"), STR("ID"))},
        {"slotNum", helper.Num()},
        {"pals", std::move(pals)},
        {"status", partial ? "partial" : "ok"},
    };
    return response;
}

amity::GameResponse snapshot_pal_detail(const nlohmann::json& args)
{
    FGuid player_uid;
    std::string error;
    if (!amity_rt::parse_player_uid(args, player_uid, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }
    std::optional<FGuid> instance_id{};
    if (args.contains("instanceId") && !args["instanceId"].is_null())
    {
        if (!args["instanceId"].is_string())
        {
            return amity::GameResponse::fail("validation_failed", "instanceId must be a string");
        }
        instance_id = parse_guid(args["instanceId"].get<std::string>());
        if (!instance_id)
        {
            return amity::GameResponse::fail("validation_failed", "instanceId is not a valid GUID");
        }
    }
    if (!instance_id &&
        (!args.contains("slotIndex") || !args["slotIndex"].is_number_integer() || args["slotIndex"].get<int64_t>() < 0))
    {
        return amity::GameResponse::fail("validation_failed", "slotIndex is required and must be a non-negative integer");
    }
    const int32_t slot_index_arg =
        instance_id ? -1 : static_cast<int32_t>(args["slotIndex"].get<int64_t>());

    UObject* world_context = any_player_controller();
    if (!world_context)
    {
        return amity::GameResponse::fail("capability_unavailable", "no live world context");
    }

    RequiredFunctions fns = resolve_functions();

    if (instance_id)
    {
        UObject* individual_parameter =
            amity_rt::parameter_by_instance_id(world_context, player_uid, *instance_id);
        if (!individual_parameter)
        {
            return amity::GameResponse::fail("validation_failed", "no pal with that instanceId");
        }
        nlohmann::json entry = summarize_pal(nullptr, individual_parameter, -1);
        add_pal_detail_fields(entry, individual_parameter);
        entry["instanceId"] = format_guid(*instance_id);
        entry["playerUid"] = format_guid(player_uid);
        entry["status"] = "ok";
        amity::GameResponse response;
        response.data = entry;
        return response;
    }

    if (!fns.get_storage || !fns.get_slot_by_index || !fns.get_handle || !fns.try_get_individual_parameter)
    {
        return amity::GameResponse::fail("capability_unavailable", "pal slot functions unresolved");
    }

    GetStorageParams storage_params{};
    storage_params.WorldContextObject = world_context;
    storage_params.PlayerUId = player_uid;
    world_context->ProcessEvent(fns.get_storage, &storage_params);
    UObject* storage = storage_params.ReturnValue;
    if (!storage)
    {
        return amity::GameResponse::fail("validation_failed", "unknown playerUid");
    }

    GetSlotBySlotIndexParams slot_params{};
    slot_params.SlotIndex = slot_index_arg;
    storage->ProcessEvent(fns.get_slot_by_index, &slot_params);
    UObject* slot = slot_params.ReturnValue;
    if (!slot)
    {
        return amity::GameResponse::fail("validation_failed", "slotIndex out of range");
    }

    NoParamsObjectReturn handle_params{};
    slot->ProcessEvent(fns.get_handle, &handle_params);
    UObject* handle = handle_params.ReturnValue;
    if (!handle)
    {
        return amity::GameResponse::fail("validation_failed", "slot is empty");
    }

    NoParamsObjectReturn param_params{};
    handle->ProcessEvent(fns.try_get_individual_parameter, &param_params);
    UObject* individual_parameter = param_params.ReturnValue;

    nlohmann::json entry = summarize_pal(handle, individual_parameter, slot_index_arg);
    add_pal_detail_fields(entry, individual_parameter);
    entry["status"] = individual_parameter ? "ok" : "partial";

    amity::GameResponse response;
    response.data = entry;
    return response;
}
}
