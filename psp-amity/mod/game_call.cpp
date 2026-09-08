#include "game_call.hpp"

using namespace RC;
using namespace RC::Unreal;

namespace
{
struct ObjectReturn
{
    UObject* ReturnValue{};
};

struct PlayerObjectParams
{
    UObject* Player{};
    UObject* ReturnValue{};
};

struct GetComponentByClassParams
{
    UClass* ComponentClass{};
    UObject* ReturnValue{};
};

UObject* transmitter_component(UObject* pawn, const wchar_t* getter_path)
{
    UObject* transmitter = amity_rt::network_transmitter(pawn);
    UFunction* getter = amity_rt::find_function(getter_path);
    if (!transmitter || !getter)
    {
        return nullptr;
    }
    ObjectReturn params{};
    transmitter->ProcessEvent(getter, &params);
    return params.ReturnValue;
}
}

namespace amity_rt
{
UObject* network_transmitter(UObject* pawn)
{
    UFunction* get_transmitter_fn = find_function(STR("/Script/Pal.PalUtility:GetNetworkTransmitterByPlayerCharacter"));
    if (!pawn || !get_transmitter_fn)
    {
        return nullptr;
    }
    PlayerObjectParams params{};
    params.Player = pawn;
    pawn->ProcessEvent(get_transmitter_fn, &params);
    return params.ReturnValue;
}

UObject* item_network_component(UObject* pawn)
{
    return transmitter_component(pawn, STR("/Script/Pal.PalNetworkTransmitter:GetItem"));
}

UObject* character_container_component(UObject* pawn)
{
    return transmitter_component(pawn, STR("/Script/Pal.PalNetworkTransmitter:GetCharacterContainer"));
}

UObject* otomo_holder(UObject* player_state)
{
    UClass* holder_class = amity_rt::find_class(STR("/Script/Pal.PalOtomoHolderComponentBase"));
    UFunction* get_component_fn = amity_rt::find_function(STR("/Script/Engine.Actor:GetComponentByClass"));
    UFunction* get_controller_fn = amity_rt::find_function(STR("/Script/Engine.PlayerState:GetPlayerController"));
    UFunction* get_pawn_fn = amity_rt::find_function(STR("/Script/Engine.Controller:K2_GetPawn"));
    if (!player_state || !holder_class || !get_component_fn || !get_controller_fn || !get_pawn_fn)
    {
        return nullptr;
    }

    ObjectReturn controller_params{};
    player_state->ProcessEvent(get_controller_fn, &controller_params);
    UObject* controller = controller_params.ReturnValue;

    UObject* pawn = nullptr;
    if (controller)
    {
        ObjectReturn pawn_params{};
        controller->ProcessEvent(get_pawn_fn, &pawn_params);
        pawn = pawn_params.ReturnValue;
    }

    for (UObject* owner : {pawn, controller, player_state})
    {
        if (!owner)
        {
            continue;
        }
        GetComponentByClassParams params{};
        params.ComponentClass = holder_class;
        owner->ProcessEvent(get_component_fn, &params);
        if (params.ReturnValue)
        {
            return params.ReturnValue;
        }
    }
    return nullptr;
}

UObject* player_pawn(UObject* player_state, const char*& reason)
{
    UFunction* get_controller_fn = find_function(STR("/Script/Engine.PlayerState:GetPlayerController"));
    UFunction* get_pawn_fn = find_function(STR("/Script/Engine.Controller:K2_GetPawn"));
    if (!player_state || !get_controller_fn || !get_pawn_fn)
    {
        reason = "player pawn functions unresolved";
        return nullptr;
    }

    ObjectReturn controller_params{};
    player_state->ProcessEvent(get_controller_fn, &controller_params);
    if (!controller_params.ReturnValue)
    {
        reason = "player controller unavailable";
        return nullptr;
    }

    ObjectReturn pawn_params{};
    controller_params.ReturnValue->ProcessEvent(get_pawn_fn, &pawn_params);
    if (!pawn_params.ReturnValue)
    {
        reason = "player is not spawned";
        return nullptr;
    }
    return pawn_params.ReturnValue;
}
bool write_instance_id(UStruct* type, void* value, const FGuid& player_uid, const FGuid& instance_id)
{
    FGuid* player = snap::value_ptr<FGuid>(find_struct_prop(type, {STR("PlayerUId"), STR("PlayerUID")}), value);
    FGuid* instance = snap::value_ptr<FGuid>(find_struct_prop(type, {STR("InstanceId"), STR("InstanceID")}), value);
    if (!player || !instance)
    {
        return false;
    }
    *player = player_uid;
    *instance = instance_id;
    return true;
}

UObject* object_return_of(ParamBlock& block)
{
    FProperty* return_prop = block.param({STR("ReturnValue")});
    auto* object_prop = CastField<FObjectProperty>(return_prop);
    return object_prop ? object_prop->GetObjectPropertyValue(block.value_of(return_prop)) : nullptr;
}

UObject* character_manager(UObject* world_context)
{
    UFunction* get_manager_fn = find_function(STR("/Script/Pal.PalUtility:GetCharacterManager"));
    if (!world_context || !get_manager_fn)
    {
        return nullptr;
    }
    snap::WorldContextObjectReturnParams params{};
    params.WorldContextObject = world_context;
    world_context->ProcessEvent(get_manager_fn, &params);
    return params.ReturnValue;
}

UObject* parameter_by_id(UObject* manager, const FGuid& player_uid, const FGuid& instance_id)
{
    UFunction* get_parameter_fn = find_function(STR("/Script/Pal.PalCharacterManager:GetIndividualCharacterParameter"));
    if (!manager || !get_parameter_fn)
    {
        return nullptr;
    }
    ParamBlock block(get_parameter_fn);
    FStructProperty* id_param = block.valid() ? struct_param(block, {STR("IndividualId"), STR("ID")}) : nullptr;
    if (!id_param || !write_instance_id(id_param->GetStruct(), block.value_of(id_param), player_uid, instance_id))
    {
        return nullptr;
    }
    block.invoke(manager);
    return object_return_of(block);
}

UObject* party_parameter_at(UObject* holder, int32_t seat)
{
    UFunction* get_handle_fn = find_function(STR("/Script/Pal.PalOtomoHolderComponentBase:GetOtomoIndividualHandle"));
    UFunction* get_parameter_fn = find_function(STR("/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter"));
    if (!holder || !get_handle_fn || !get_parameter_fn)
    {
        return nullptr;
    }
    struct OtomoHandleParams
    {
        int32_t SlotIndex{};
        UObject* ReturnValue{};
    } handle_params{};
    handle_params.SlotIndex = seat;
    holder->ProcessEvent(get_handle_fn, &handle_params);
    if (!handle_params.ReturnValue)
    {
        return nullptr;
    }
    struct NoParamsObjectReturn
    {
        UObject* ReturnValue{};
    } parameter_params{};
    handle_params.ReturnValue->ProcessEvent(get_parameter_fn, &parameter_params);
    return parameter_params.ReturnValue;
}

std::string instance_id_of_parameter(UObject* parameter)
{
    auto* handle_prop = parameter ? CastField<FStructProperty>(find_prop(parameter, {STR("IndividualId")})) : nullptr;
    UStruct* handle_struct = handle_prop ? handle_prop->GetStruct() : nullptr;
    if (!handle_struct || handle_struct->GetName() != STR("PalInstanceID"))
    {
        return {};
    }
    void* handle = handle_prop->ContainerPtrToValuePtr<void>(parameter);
    FGuid* instance = snap::value_ptr<FGuid>(find_struct_prop(handle_struct, {STR("InstanceId")}), handle);
    return instance ? snap::format_guid(*instance) : std::string{};
}

UObject* base_camp_container(const FGuid& base_id, const char*& reason)
{
    std::vector<UObject*> models{};
    UObjectGlobals::FindAllOf(STR("PalBaseCampModel"), models);
    for (UObject* model : models)
    {
        if (!model)
        {
            continue;
        }
        FGuid* id = snap::value_ptr<FGuid>(find_prop(model, {STR("ID")}), model);
        if (!id || !(*id == base_id))
        {
            continue;
        }
        UObject* director = snap::object_member(model, STR("WorkerDirector"));
        UObject* container = director ? snap::object_member(director, STR("CharacterContainer")) : nullptr;
        if (!container)
        {
            reason = "base camp character container unavailable";
        }
        return container;
    }
    reason = "unknown baseId";
    return nullptr;
}

UObject* base_container_slot(UObject* container, int32_t slot_index)
{
    auto* slot_array = CastField<FArrayProperty>(find_prop(container, {STR("SlotArray")}));
    auto* slot_inner = slot_array ? CastField<FObjectProperty>(slot_array->GetInner()) : nullptr;
    if (!slot_inner || slot_inner->GetSize() != static_cast<int32_t>(sizeof(UObject*)))
    {
        return nullptr;
    }
    FScriptArrayHelper helper(slot_array, slot_array->ContainerPtrToValuePtr<void>(container));
    UObject* by_position = nullptr;
    for (int32_t i = 0; i < helper.Num(); ++i)
    {
        uint8_t* element = helper.GetRawPtr(i);
        UObject* slot = element ? slot_inner->GetObjectPropertyValue(element) : nullptr;
        if (!slot)
        {
            continue;
        }
        if (i == slot_index)
        {
            by_position = slot;
        }
        const auto index = snap::read_numeric(find_prop(slot, {STR("SlotIndex")}), slot);
        if (index && static_cast<int32_t>(*index) == slot_index)
        {
            return slot;
        }
    }
    return by_position;
}

UObject* parameter_by_instance_id(UObject* world_context, const FGuid& player_uid, const FGuid& instance_id)
{
    if (!world_context)
    {
        return nullptr;
    }
    UObject* manager = character_manager(world_context);
    if (UObject* parameter = parameter_by_id(manager, player_uid, instance_id))
    {
        return parameter;
    }

    const std::string wanted = snap::format_guid(instance_id);
    UObject* holder = otomo_holder(player_state_by_uid(world_context, player_uid));
    for (int32_t seat = 0; seat < 5; ++seat)
    {
        UObject* parameter = party_parameter_at(holder, seat);
        if (parameter && instance_id_of_parameter(parameter) == wanted)
        {
            return parameter;
        }
    }

    return parameter_by_id(manager, FGuid{}, instance_id);
}

nlohmann::json revive_parameter(UObject* parameter)
{
    nlohmann::json steps = nlohmann::json::object();
    if (!parameter)
    {
        return steps;
    }

    struct NoParams
    {
        uint8_t reserved{};
    };
    struct FloatReturn
    {
        float ReturnValue{};
    };
    struct SetFullStomachParams
    {
        float NextValue{};
        bool bOverMaxStomach{};
    };
    struct SetPhysicalHealthParams
    {
        uint8_t PhysicalHealth{};
    };

    UFunction* set_physical_health = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:SetPhysicalHealth"));
    std::optional<int64_t> healthful = enum_value_by_name(STR("/Script/Pal.EPalStatusPhysicalHealthType"), STR("Healthful"));
    if (set_physical_health && healthful && *healthful >= 0 && *healthful <= 255)
    {
        SetPhysicalHealthParams params{};
        params.PhysicalHealth = static_cast<uint8_t>(*healthful);
        parameter->ProcessEvent(set_physical_health, &params);
        steps["physicalHealth"] = "ok";
    }
    else
    {
        steps["physicalHealth"] = "unresolved";
    }

    if (UFunction* full_recovery = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:FullRecoveryHP")))
    {
        NoParams params{};
        parameter->ProcessEvent(full_recovery, &params);
        steps["hp"] = "ok";
    }
    else
    {
        steps["hp"] = "unresolved";
    }

    UFunction* get_max_stomach = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:GetMaxFullStomach"));
    UFunction* set_stomach = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:SetFullStomach"));
    if (get_max_stomach && set_stomach)
    {
        FloatReturn max_stomach{};
        parameter->ProcessEvent(get_max_stomach, &max_stomach);
        SetFullStomachParams params{};
        params.NextValue = max_stomach.ReturnValue;
        parameter->ProcessEvent(set_stomach, &params);
        steps["stomach"] = "ok";
    }
    else
    {
        steps["stomach"] = "unresolved";
    }

    return steps;
}

bool all_steps_ok(const nlohmann::json& steps)
{
    if (!steps.is_object() || steps.empty())
    {
        return false;
    }
    for (const auto& [name, state] : steps.items())
    {
        if (state != "ok")
        {
            return false;
        }
    }
    return true;
}
}
