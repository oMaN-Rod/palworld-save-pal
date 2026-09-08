#pragma once
#include <cstdint>
#include <initializer_list>
#include <optional>
#include <string>

#include <Unreal/CoreUObject/UObject/Class.hpp>
#include <Unreal/CoreUObject/UObject/UnrealType.hpp>
#include <Unreal/UObject.hpp>
#include <Unreal/UObjectGlobals.hpp>
#include <Unreal/UnrealCoreStructs.hpp>

namespace amity_rt
{
std::wstring widen(const std::string& s);

RC::Unreal::UFunction* find_function(const wchar_t* full_path);
RC::Unreal::UClass* find_class(const wchar_t* full_path);
RC::Unreal::UEnum* find_enum(const wchar_t* full_path);
RC::Unreal::UScriptStruct* find_script_struct(const wchar_t* full_path);
RC::Unreal::FProperty* find_prop(RC::Unreal::UObject* obj, std::initializer_list<const wchar_t*> aliases);
RC::Unreal::FProperty* find_struct_prop(RC::Unreal::UStruct* strct, std::initializer_list<const wchar_t*> aliases);
RC::Unreal::UObject* any_player_controller();
bool world_ready();

std::optional<int64_t> enum_value_by_name(const wchar_t* enum_path, const wchar_t* value_name);
std::optional<std::wstring> enum_name_by_value(const wchar_t* enum_path, int64_t value);

void resolve_mode(std::string& mode, bool& authoritative);

RC::Unreal::UObject* player_state_by_uid(RC::Unreal::UObject* world_context, const RC::Unreal::FGuid& player_uid);

RC::Unreal::UObject* palbox_slot_by_index(RC::Unreal::UObject* world_context,
                                           const RC::Unreal::FGuid& player_uid,
                                           int32_t slot_index,
                                           std::string& error_code,
                                           std::string& error_message);

RC::Unreal::UObject* individual_parameter_by_slot_index(RC::Unreal::UObject* world_context,
                                                         const RC::Unreal::FGuid& player_uid,
                                                         int32_t slot_index,
                                                         std::string& error_code,
                                                         std::string& error_message);

// Weak pointers need their own branch: FWeakObjectProperty's cast flags do not carry the
// FObjectPropertyBase bit, so CastField<FObjectPropertyBase> misses them entirely.
std::wstring declared_type_name(RC::Unreal::FProperty* prop);
}
