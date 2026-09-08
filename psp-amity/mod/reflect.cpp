#include "reflect.hpp"

#include <Unreal/Core/Containers/Array.hpp>
#include <Unreal/NameTypes.hpp>

#include <utility>
#include <vector>

using namespace RC;
using namespace RC::Unreal;

namespace
{
struct WorldContextBoolParams
{
    UObject* WorldContextObject{};
    bool ReturnValue{};
};

struct GetStorageParams
{
    UObject* WorldContextObject{};
    FGuid PlayerUId{};
    UObject* ReturnValue{};
};

struct GetAllPlayerStatesParams
{
    UObject* WorldContextObject{};
    TArray<UObject*> OutPlayerStates{};
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

bool call_world_context_bool(UObject* world_context, UFunction* function)
{
    WorldContextBoolParams params{};
    params.WorldContextObject = world_context;
    world_context->ProcessEvent(function, &params);
    return params.ReturnValue;
}

bool enum_name_matches(const std::wstring& entry, const wchar_t* wanted)
{
    if (entry == wanted)
    {
        return true;
    }
    const size_t sep = entry.rfind(L"::");
    return sep != std::wstring::npos && entry.compare(sep + 2, std::wstring::npos, wanted) == 0;
}

std::wstring short_enum_name(const std::wstring& entry)
{
    const size_t sep = entry.rfind(L"::");
    return sep == std::wstring::npos ? entry : entry.substr(sep + 2);
}
}

namespace amity_rt
{
std::wstring widen(const std::string& s)
{
    return std::wstring(s.begin(), s.end());
}

UFunction* find_function(const wchar_t* full_path)
{
    return UObjectGlobals::StaticFindObject<UFunction*>(nullptr, nullptr, full_path);
}

UClass* find_class(const wchar_t* full_path)
{
    return UObjectGlobals::StaticFindObject<UClass*>(nullptr, nullptr, full_path);
}

UEnum* find_enum(const wchar_t* full_path)
{
    return UObjectGlobals::StaticFindObject<UEnum*>(nullptr, nullptr, full_path);
}

UScriptStruct* find_script_struct(const wchar_t* full_path)
{
    return UObjectGlobals::StaticFindObject<UScriptStruct*>(nullptr, nullptr, full_path);
}

FProperty* find_prop(UObject* obj, std::initializer_list<const wchar_t*> aliases)
{
    if (!obj)
    {
        return nullptr;
    }
    for (const wchar_t* alias : aliases)
    {
        if (FProperty* prop = obj->GetPropertyByNameInChain(alias))
        {
            return prop;
        }
    }
    return nullptr;
}

FProperty* find_struct_prop(UStruct* strct, std::initializer_list<const wchar_t*> aliases)
{
    if (!strct)
    {
        return nullptr;
    }
    for (const wchar_t* alias : aliases)
    {
        if (FProperty* prop = strct->GetPropertyByNameInChain(alias))
        {
            return prop;
        }
    }
    return nullptr;
}

UObject* any_player_controller()
{
    std::vector<UObject*> controllers{};
    UObjectGlobals::FindAllOf(STR("PalPlayerController"), controllers);
    for (UObject* controller : controllers)
    {
        if (controller)
        {
            return controller;
        }
    }
    return nullptr;
}

bool world_ready()
{
    std::vector<UObject*> player_states{};
    UObjectGlobals::FindAllOf(STR("PalPlayerState"), player_states);
    for (UObject* player_state : player_states)
    {
        if (player_state)
        {
            return true;
        }
    }
    return false;
}

std::optional<int64_t> enum_value_by_name(const wchar_t* enum_path, const wchar_t* value_name)
{
    UEnum* enum_type = find_enum(enum_path);
    if (!enum_type)
    {
        return std::nullopt;
    }
    std::vector<std::pair<FName, int64_t>> names{};
    enum_type->GetEnumNamesAsVector(names);
    for (const auto& [name, value] : names)
    {
        if (enum_name_matches(name.ToString(), value_name))
        {
            return value;
        }
    }
    return std::nullopt;
}

std::optional<std::wstring> enum_name_by_value(const wchar_t* enum_path, int64_t value)
{
    UEnum* enum_type = find_enum(enum_path);
    if (!enum_type)
    {
        return std::nullopt;
    }
    std::vector<std::pair<FName, int64_t>> names{};
    enum_type->GetEnumNamesAsVector(names);
    for (const auto& [name, entry_value] : names)
    {
        if (entry_value == value)
        {
            return short_enum_name(name.ToString());
        }
    }
    return std::nullopt;
}

void resolve_mode(std::string& mode, bool& authoritative)
{
    mode = "unknown";
    authoritative = false;

    if (!world_ready())
    {
        return;
    }

    UObject* world_context = any_player_controller();
    if (!world_context)
    {
        return;
    }

    UFunction* is_standalone_fn = find_function(STR("/Script/Engine.KismetSystemLibrary:IsStandalone"));
    UFunction* is_dedicated_fn = find_function(STR("/Script/Engine.KismetSystemLibrary:IsDedicatedServer"));
    UFunction* is_server_fn = find_function(STR("/Script/Engine.KismetSystemLibrary:IsServer"));
    if (!is_standalone_fn || !is_dedicated_fn || !is_server_fn)
    {
        return;
    }

    if (call_world_context_bool(world_context, is_standalone_fn))
    {
        mode = "solo";
        authoritative = true;
    }
    else if (call_world_context_bool(world_context, is_dedicated_fn))
    {
        mode = "dedicated";
        authoritative = true;
    }
    else if (call_world_context_bool(world_context, is_server_fn))
    {
        mode = "coop_host";
        authoritative = true;
    }
    else
    {
        mode = "coop_client";
        authoritative = false;
    }
}

UObject* player_state_by_uid(UObject* world_context, const FGuid& player_uid)
{
    UFunction* get_all_fn = find_function(STR("/Script/Pal.PalUtility:GetAllPlayerStates"));
    if (!world_context || !get_all_fn)
    {
        return nullptr;
    }

    GetAllPlayerStatesParams params{};
    params.WorldContextObject = world_context;
    world_context->ProcessEvent(get_all_fn, &params);

    const int32_t count = params.OutPlayerStates.Num();
    for (int32_t i = 0; i < count; ++i)
    {
        UObject* player_state = params.OutPlayerStates[i];
        if (!player_state)
        {
            continue;
        }
        FProperty* uid_prop = find_prop(player_state, {STR("PlayerUId")});
        if (!uid_prop)
        {
            continue;
        }
        if (*uid_prop->ContainerPtrToValuePtr<FGuid>(player_state) == player_uid)
        {
            return player_state;
        }
    }
    return nullptr;
}

UObject* palbox_slot_by_index(UObject* world_context,
                               const FGuid& player_uid,
                               int32_t slot_index,
                               std::string& error_code,
                               std::string& error_message)
{
    UFunction* get_storage_fn = find_function(STR("/Script/Pal.PalUtility:GetPalStorageDataByPlayerUID"));
    UFunction* get_slot_fn = find_function(STR("/Script/Pal.PalPlayerDataPalStorage:GetSlotBySlotIndex"));
    if (!world_context || !get_storage_fn || !get_slot_fn)
    {
        error_code = "capability_unavailable";
        error_message = "pal storage functions unresolved";
        return nullptr;
    }

    GetStorageParams storage_params{};
    storage_params.WorldContextObject = world_context;
    storage_params.PlayerUId = player_uid;
    world_context->ProcessEvent(get_storage_fn, &storage_params);
    UObject* storage = storage_params.ReturnValue;
    if (!storage)
    {
        error_code = "validation_failed";
        error_message = "unknown playerUid";
        return nullptr;
    }

    GetSlotBySlotIndexParams slot_params{};
    slot_params.SlotIndex = slot_index;
    storage->ProcessEvent(get_slot_fn, &slot_params);
    if (!slot_params.ReturnValue)
    {
        error_code = "validation_failed";
        error_message = "slotIndex out of range";
        return nullptr;
    }
    return slot_params.ReturnValue;
}

UObject* individual_parameter_by_slot_index(UObject* world_context,
                                             const FGuid& player_uid,
                                             int32_t slot_index,
                                             std::string& error_code,
                                             std::string& error_message)
{
    UFunction* get_handle_fn = find_function(STR("/Script/Pal.PalIndividualCharacterSlot:GetHandle"));
    UFunction* get_parameter_fn = find_function(STR("/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter"));
    if (!get_handle_fn || !get_parameter_fn)
    {
        error_code = "capability_unavailable";
        error_message = "pal storage functions unresolved";
        return nullptr;
    }

    UObject* slot = palbox_slot_by_index(world_context, player_uid, slot_index, error_code, error_message);
    if (!slot)
    {
        return nullptr;
    }

    NoParamsObjectReturn handle_params{};
    slot->ProcessEvent(get_handle_fn, &handle_params);
    UObject* handle = handle_params.ReturnValue;
    if (!handle)
    {
        error_code = "validation_failed";
        error_message = "slot is empty";
        return nullptr;
    }

    NoParamsObjectReturn parameter_params{};
    handle->ProcessEvent(get_parameter_fn, &parameter_params);
    if (!parameter_params.ReturnValue)
    {
        error_code = "validation_failed";
        error_message = "pal parameter not loaded";
        return nullptr;
    }
    return parameter_params.ReturnValue;
}

std::wstring describe_property(FProperty* prop);

std::wstring declared_type_name(FProperty* prop)
{
    if (auto* object_prop = CastField<FObjectPropertyBase>(prop))
    {
        UClass* declared_class = object_prop->GetPropertyClass().Get();
        return declared_class ? std::wstring(declared_class->GetFullName()) : std::wstring{};
    }
    if (auto* struct_prop = CastField<FStructProperty>(prop))
    {
        UStruct* declared_struct = struct_prop->GetStruct();
        return declared_struct ? std::wstring(declared_struct->GetName()) : std::wstring{};
    }
    // A weak pointer's cast flags do not carry the FObjectPropertyBase bit, so the CastField
    // above misses it.
    if (auto* weak_prop = CastField<FWeakObjectProperty>(prop))
    {
        UClass* declared_class = weak_prop->GetPropertyClass().Get();
        return declared_class ? std::wstring(declared_class->GetFullName()) : std::wstring{};
    }
    if (auto* array_prop = CastField<FArrayProperty>(prop))
    {
        FProperty* inner = array_prop->GetInner();
        if (!inner)
        {
            return std::wstring{};
        }
        std::wstring text = STR("TArray<");
        text += describe_property(inner);
        text += STR(">");
        return text;
    }
    if (auto* map_prop = CastField<FMapProperty>(prop))
    {
        FProperty* key = map_prop->GetKeyProp();
        FProperty* value = map_prop->GetValueProp();
        if (!key || !value)
        {
            return std::wstring{};
        }
        std::wstring text = STR("TMap<");
        text += describe_property(key);
        text += STR(", ");
        text += describe_property(value);
        text += STR(">");
        return text;
    }
    return std::wstring{};
}

std::wstring describe_property(FProperty* prop)
{
    std::wstring text = prop->GetClass().GetName();
    const std::wstring declared = declared_type_name(prop);
    if (!declared.empty())
    {
        text += STR(" ");
        text += declared;
    }
    return text;
}
}
