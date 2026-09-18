#pragma once

#include "reflect.hpp"

#include <Unreal/Core/Containers/Array.hpp>
#include <Unreal/Core/Containers/StringConv.hpp>
#include <Unreal/Core/Containers/UnrealString.hpp>
#include <Unreal/NameTypes.hpp>
#include <Unreal/Property/FEnumProperty.hpp>
#include <Unreal/Rotator.hpp>
#include <Unreal/UnrealCoreStructs.hpp>

#include <amity/game_port.hpp>

#include <cstdint>
#include <cstring>
#include <optional>
#include <string>
#include <vector>

namespace amity_rt
{
amity::GameResponse snapshot_players();
amity::GameResponse snapshot_pals(const nlohmann::json& args);
amity::GameResponse snapshot_pal_detail(const nlohmann::json& args);
amity::GameResponse snapshot_inventory(const nlohmann::json& args);
amity::GameResponse snapshot_guild(const nlohmann::json& args);
amity::GameResponse snapshot_guilds(const nlohmann::json& args);
amity::GameResponse snapshot_base_pals(const nlohmann::json& args);
amity::GameResponse snapshot_guild_containers(const nlohmann::json& args);
amity::GameResponse snapshot_reflect(const nlohmann::json& args);
amity::GameResponse snapshot_build_info();
amity::GameResponse snapshot_loaded_mods();

namespace snap
{
using namespace RC;
using namespace RC::Unreal;

inline std::string to_utf8(const wchar_t* wide)
{
    if (!wide || !*wide)
    {
        return {};
    }
    auto conv = RC::Unreal::StringCast<char8_t>(wide);
    return std::string(reinterpret_cast<const char*>(conv.Get()), static_cast<size_t>(conv.Length()));
}

inline std::string to_utf8(const std::wstring& wide)
{
    return to_utf8(wide.c_str());
}

// The object an object-typed property points at. Weak properties are deliberately not
// handled: their storage is an index and serial, not an address, and reading one as an
// address is a crash rather than a wrong answer.
inline UObject* object_member(UObject* owner, const wchar_t* name)
{
    auto* prop = CastField<FObjectPropertyBase>(amity_rt::find_prop(owner, {name}));
    return prop ? prop->GetObjectPropertyValue(prop->ContainerPtrToValuePtr<void>(owner)) : nullptr;
}

// Canonical lowercase 8-4-4-4-12 form of the raw A/B/C/D words, matching UE's
// FGuid::ToString(EGuidFormats::DigitsWithHyphens) grouping (lowercased).
inline std::string format_guid(const FGuid& guid)
{
    char buf[37];
    std::snprintf(buf,
                  sizeof(buf),
                  "%08x-%04x-%04x-%04x-%04x%08x",
                  guid.A,
                  (guid.B >> 16) & 0xFFFFu,
                  guid.B & 0xFFFFu,
                  (guid.C >> 16) & 0xFFFFu,
                  guid.C & 0xFFFFu,
                  guid.D);
    return std::string(buf);
}

inline std::optional<FGuid> parse_guid(const std::string& s)
{
    if (s.size() != 36 || s[8] != '-' || s[13] != '-' || s[18] != '-' || s[23] != '-')
    {
        return std::nullopt;
    }
    auto hex_group = [&](size_t pos, size_t len) -> std::optional<uint64_t> {
        uint64_t v = 0;
        for (size_t i = 0; i < len; ++i)
        {
            const char c = s[pos + i];
            uint64_t digit;
            if (c >= '0' && c <= '9')
            {
                digit = static_cast<uint64_t>(c - '0');
            }
            else if (c >= 'a' && c <= 'f')
            {
                digit = 10u + static_cast<uint64_t>(c - 'a');
            }
            else if (c >= 'A' && c <= 'F')
            {
                digit = 10u + static_cast<uint64_t>(c - 'A');
            }
            else
            {
                return std::nullopt;
            }
            v = (v << 4) | digit;
        }
        return v;
    };

    const auto a = hex_group(0, 8);
    const auto b1 = hex_group(9, 4);
    const auto b2 = hex_group(14, 4);
    const auto c1 = hex_group(19, 4);
    const auto c2 = hex_group(24, 4);
    const auto d = hex_group(28, 8);
    if (!a || !b1 || !b2 || !c1 || !c2 || !d)
    {
        return std::nullopt;
    }

    FGuid guid;
    guid.A = static_cast<uint32_t>(*a);
    guid.B = static_cast<uint32_t>((*b1 << 16) | *b2);
    guid.C = static_cast<uint32_t>((*c1 << 16) | *c2);
    guid.D = static_cast<uint32_t>(*d);
    return guid;
}

inline std::optional<std::string> gender_name(int64_t value)
{
    switch (value)
    {
    case 0:
        return std::string("none");
    case 1:
        return std::string("male");
    case 2:
        return std::string("female");
    default:
        return std::nullopt;
    }
}

template <typename T>
inline T* value_ptr(FProperty* prop, UObject* container)
{
    return (prop && container) ? prop->ContainerPtrToValuePtr<T>(container) : nullptr;
}

template <typename T>
inline T* value_ptr(FProperty* prop, void* container)
{
    return (prop && container) ? prop->ContainerPtrToValuePtr<T>(container) : nullptr;
}

inline nlohmann::json nested_guid(UObject* owner, const wchar_t* member, const wchar_t* inner)
{
    auto* struct_prop = CastField<FStructProperty>(amity_rt::find_prop(owner, {member}));
    if (!struct_prop || !struct_prop->GetStruct())
    {
        return nullptr;
    }
    void* ptr = struct_prop->ContainerPtrToValuePtr<void>(owner);
    if (FGuid* guid = value_ptr<FGuid>(amity_rt::find_struct_prop(struct_prop->GetStruct(), {inner}), ptr))
    {
        return format_guid(*guid);
    }
    return nullptr;
}

inline std::optional<bool> read_bool(FProperty* prop, void* container)
{
    if (auto* bool_prop = CastField<FBoolProperty>(prop); bool_prop && container)
    {
        return bool_prop->GetPropertyValueInContainer(container);
    }
    return std::nullopt;
}

// The mirror of read_numeric: an integral or enum leaf written through the property system, so
// a leaf that is neither refuses instead of having bytes poked into it.
// `container` is the OWNER of the property -- an object, or the base of a struct -- not a
// pointer to the value itself. This offsets it; offsetting again before the call writes into
// whatever field comes next.
inline bool write_integral(FProperty* prop, void* container, int64_t value)
{
    if (!prop || !container)
    {
        return false;
    }
    if (auto* enum_prop = CastField<FEnumProperty>(prop))
    {
        FNumericProperty* underlying = enum_prop->GetUnderlyingProperty();
        if (!underlying)
        {
            return false;
        }
        underlying->SetIntPropertyValue(enum_prop->ContainerPtrToValuePtr<void>(container), value);
        return true;
    }
    if (auto* numeric = CastField<FNumericProperty>(prop); numeric && !numeric->IsFloatingPoint())
    {
        numeric->SetIntPropertyValue(numeric->ContainerPtrToValuePtr<void>(container), value);
        return true;
    }
    return false;
}

inline std::optional<int64_t> read_numeric(FProperty* prop, void* container)
{
    if (!prop || !container)
    {
        return std::nullopt;
    }
    if (auto* enum_prop = CastField<FEnumProperty>(prop))
    {
        FNumericProperty* underlying = enum_prop->GetUnderlyingProperty();
        if (!underlying)
        {
            return std::nullopt;
        }
        void* ptr = enum_prop->ContainerPtrToValuePtr<void>(container);
        return underlying->GetSignedIntPropertyValue(ptr);
    }
    if (auto* numeric_prop = CastField<FNumericProperty>(prop))
    {
        void* ptr = numeric_prop->ContainerPtrToValuePtr<void>(container);
        return numeric_prop->GetSignedIntPropertyValue(ptr);
    }
    return std::nullopt;
}

inline std::optional<double> read_float(FProperty* prop, void* container)
{
    if (!prop || !container)
    {
        return std::nullopt;
    }
    if (auto* float_prop = CastField<FFloatProperty>(prop))
    {
        if (float_prop->GetSize() != static_cast<int32_t>(sizeof(float)))
        {
            return std::nullopt;
        }
        return static_cast<double>(*float_prop->ContainerPtrToValuePtr<float>(container));
    }
    if (auto* double_prop = CastField<FDoubleProperty>(prop))
    {
        if (double_prop->GetSize() != static_cast<int32_t>(sizeof(double)))
        {
            return std::nullopt;
        }
        return *double_prop->ContainerPtrToValuePtr<double>(container);
    }
    return std::nullopt;
}

inline bool write_float(FProperty* prop, void* container, double value)
{
    if (!prop || !container)
    {
        return false;
    }
    if (auto* float_prop = CastField<FFloatProperty>(prop))
    {
        if (float_prop->GetSize() != static_cast<int32_t>(sizeof(float)))
        {
            return false;
        }
        *float_prop->ContainerPtrToValuePtr<float>(container) = static_cast<float>(value);
        return true;
    }
    if (auto* double_prop = CastField<FDoubleProperty>(prop))
    {
        if (double_prop->GetSize() != static_cast<int32_t>(sizeof(double)))
        {
            return false;
        }
        *double_prop->ContainerPtrToValuePtr<double>(container) = value;
        return true;
    }
    return false;
}

// HP and its maximum are FFixedPoint64, a struct wrapping one int64 `Value` scaled by 1000.
inline std::optional<int64_t> read_fixed_point(FProperty* prop, void* container)
{
    auto* struct_prop = CastField<FStructProperty>(prop);
    if (!struct_prop || !container)
    {
        return std::nullopt;
    }
    void* inner = struct_prop->ContainerPtrToValuePtr<void>(container);
    if (int64_t* value = value_ptr<int64_t>(find_struct_prop(struct_prop->GetStruct(), {STR("Value")}), inner))
    {
        return *value;
    }
    return std::nullopt;
}

inline bool write_fixed_point(FProperty* prop, void* container, int64_t value)
{
    auto* struct_prop = CastField<FStructProperty>(prop);
    if (!struct_prop || !container)
    {
        return false;
    }
    void* inner = struct_prop->ContainerPtrToValuePtr<void>(container);
    return write_integral(find_struct_prop(struct_prop->GetStruct(), {STR("Value")}), inner, value);
}

inline std::string bare_enumerator(const std::string& qualified)
{
    const size_t sep = qualified.rfind("::");
    return sep == std::string::npos ? qualified : qualified.substr(sep + 2);
}

// Every enumerator an enum defines, by scanning values rather than walking its name table:
// UE4SS exposes that table through version-gated getters, while `GetNameByValue` is the same
// accessor the snapshot readers already rely on. `None` marks an unused value, so it is the
// stop condition rather than an entry.
inline nlohmann::json enum_names(UEnum* enum_type, int64_t limit = 32)
{
    if (!enum_type)
    {
        return nullptr;
    }
    nlohmann::json out = nlohmann::json::array();
    for (int64_t value = 0; value < limit; ++value)
    {
        const std::string name = bare_enumerator(to_utf8(enum_type->GetNameByValue(value).ToString()));
        const bool is_max = name.size() > 4 && name.compare(name.size() - 4, 4, "_MAX") == 0;
        if (name.empty() || name == "None" || is_max)
        {
            continue;
        }
        out.push_back(name);
    }
    return out;
}

inline nlohmann::json read_work_suitability(FProperty* prop, void* container)
{
    nlohmann::json out = nlohmann::json::object();
    auto* array_prop = CastField<FArrayProperty>(prop);
    auto* inner = array_prop ? CastField<FStructProperty>(array_prop->GetInner()) : nullptr;
    if (!inner || !container)
    {
        return out;
    }
    UStruct* entry_type = inner->GetStruct();
    FProperty* suitability_prop = find_struct_prop(entry_type, {STR("WorkSuitability")});
    FProperty* rank_prop = find_struct_prop(entry_type, {STR("Rank")});
    UEnum* enum_type = nullptr;
    if (auto* enum_prop = CastField<FEnumProperty>(suitability_prop))
    {
        enum_type = enum_prop->GetEnum().Get();
    }
    else if (auto* numeric = CastField<FNumericProperty>(suitability_prop))
    {
        enum_type = numeric->GetIntPropertyEnum();
    }
    if (!enum_type || !rank_prop)
    {
        return out;
    }

    FScriptArrayHelper helper(array_prop, array_prop->ContainerPtrToValuePtr<void>(container));
    for (int32_t i = 0; i < helper.Num(); ++i)
    {
        uint8_t* entry = helper.GetRawPtr(i);
        if (!entry)
        {
            continue;
        }
        const auto suitability = read_numeric(suitability_prop, entry);
        const auto rank = read_numeric(rank_prop, entry);
        if (!suitability || !rank)
        {
            continue;
        }
        out[bare_enumerator(to_utf8(enum_type->GetNameByValue(*suitability).ToString()))] = *rank;
    }
    return out;
}

inline std::vector<std::string> read_name_array(FProperty* prop, void* container)
{
    std::vector<std::string> out;
    auto* array_prop = CastField<FArrayProperty>(prop);
    if (!array_prop || !container || !CastField<FNameProperty>(array_prop->GetInner()))
    {
        return out;
    }
    void* array_ptr = array_prop->ContainerPtrToValuePtr<void>(container);
    FScriptArrayHelper helper(array_prop, array_ptr);
    for (int32_t i = 0; i < helper.Num(); ++i)
    {
        if (auto* elem = reinterpret_cast<FName*>(helper.GetRawPtr(i)))
        {
            out.push_back(to_utf8(elem->ToString()));
        }
    }
    return out;
}

inline std::vector<std::string> read_enum_array(FProperty* prop, void* container)
{
    std::vector<std::string> out;
    auto* array_prop = CastField<FArrayProperty>(prop);
    if (!array_prop || !container)
    {
        return out;
    }

    FProperty* inner = array_prop->GetInner();
    FNumericProperty* numeric = nullptr;
    UEnum* enum_type = nullptr;
    if (auto* enum_prop = CastField<FEnumProperty>(inner))
    {
        numeric = enum_prop->GetUnderlyingProperty();
        enum_type = enum_prop->GetEnum().Get();
    }
    else if (auto* numeric_inner = CastField<FNumericProperty>(inner))
    {
        numeric = numeric_inner;
        enum_type = numeric_inner->GetIntPropertyEnum();
    }
    if (!numeric || !enum_type)
    {
        return out;
    }

    void* array_ptr = array_prop->ContainerPtrToValuePtr<void>(container);
    FScriptArrayHelper helper(array_prop, array_ptr);
    for (int32_t i = 0; i < helper.Num(); ++i)
    {
        uint8_t* elem_ptr = helper.GetRawPtr(i);
        if (!elem_ptr)
        {
            continue;
        }
        const int64_t value = numeric->GetSignedIntPropertyValue(elem_ptr);
        out.push_back(to_utf8(enum_type->GetNameByValue(value).ToString()));
    }
    return out;
}

// Calls a UFUNCTION returning a UObject* whose sole parameter is `param_prop`'s struct type,
// passed by value, without needing that struct's C++ shape at compile time. A struct like
// FPalInstanceID is not POD (it owns an FString DebugName): never raw-memcpy its bytes into a
// ProcessEvent params buffer — the source bytes are still owned by the live object they came
// from, and the engine destructs the params buffer's properties after the call, so a memcpy'd
// buffer aliasing that same heap allocation is a use-after-free/double-free waiting to happen.
// Instead, the buffer is default-constructed via the property system (FProperty::InitializeValue,
// giving DebugName a valid empty FString) and then deep-copied from `src_value_ptr` via
// FProperty::CopyCompleteValue, which performs real FString/TArray copy-construction (a fresh
// allocation) rather than a bitwise copy.
inline UObject* call_object_return_with_struct_param(UObject* target, UFunction* fn, FProperty* param_prop, const void* src_value_ptr)
{
    if (!target || !fn || !param_prop || !src_value_ptr)
    {
        return nullptr;
    }
    const int32_t param_size = param_prop->GetSize();
    if (param_size <= 0 || param_size > 512)
    {
        return nullptr;
    }
    auto* return_prop = CastField<FObjectProperty>(find_struct_prop(fn, {STR("ReturnValue")}));
    if (!return_prop || return_prop->GetSize() != static_cast<int32_t>(sizeof(UObject*)) ||
        return_prop->GetOffset_ForInternal() < param_size)
    {
        return nullptr;
    }
    const size_t return_offset = static_cast<size_t>(return_prop->GetOffset_ForInternal());
    std::vector<uint8_t> buffer(return_offset + sizeof(UObject*), 0);
    param_prop->InitializeValue(buffer.data());
    param_prop->CopyCompleteValue(buffer.data(), src_value_ptr);
    target->ProcessEvent(fn, buffer.data());
    UObject* result = nullptr;
    std::memcpy(&result, buffer.data() + return_offset, sizeof(UObject*));
    return result;
}

struct WorldContextObjectReturnParams
{
    UObject* WorldContextObject{};
    UObject* ReturnValue{};
};

struct SaveParameterView
{
    void* ptr{};
    UStruct* type{};
};

inline SaveParameterView save_parameter_of(UObject* parameter)
{
    SaveParameterView view{};
    if (auto* prop = CastField<FStructProperty>(find_prop(parameter, {STR("SaveParameter")})))
    {
        view.ptr = prop->ContainerPtrToValuePtr<void>(parameter);
        view.type = prop->GetStruct();
    }
    return view;
}

inline SaveParameterView player_save_parameter(UObject* world_context,
                                                UObject* player_state,
                                                UFunction* get_char_manager_fn,
                                                UFunction* get_individual_param_fn)
{
    SaveParameterView view{};
    if (!world_context || !player_state || !get_char_manager_fn || !get_individual_param_fn)
    {
        return view;
    }

    auto* handle_struct_prop = CastField<FStructProperty>(find_prop(player_state, {STR("IndividualHandleId")}));
    UStruct* handle_struct = handle_struct_prop ? handle_struct_prop->GetStruct() : nullptr;
    if (!handle_struct_prop || !handle_struct || handle_struct->GetName() != STR("PalInstanceID"))
    {
        return view;
    }
    void* handle_ptr = handle_struct_prop->ContainerPtrToValuePtr<void>(player_state);

    WorldContextObjectReturnParams mgr_params{};
    mgr_params.WorldContextObject = world_context;
    world_context->ProcessEvent(get_char_manager_fn, &mgr_params);
    if (!mgr_params.ReturnValue)
    {
        return view;
    }

    UObject* individual_parameter = call_object_return_with_struct_param(mgr_params.ReturnValue,
                                                                         get_individual_param_fn,
                                                                         handle_struct_prop,
                                                                         handle_ptr);
    if (!individual_parameter)
    {
        return view;
    }

    auto* save_parameter_prop = CastField<FStructProperty>(find_prop(individual_parameter, {STR("SaveParameter")}));
    if (!save_parameter_prop)
    {
        return view;
    }
    view.ptr = save_parameter_prop->ContainerPtrToValuePtr<void>(individual_parameter);
    view.type = save_parameter_prop->GetStruct();
    return view;
}

inline std::optional<int64_t> read_player_exp(const SaveParameterView& save_parameter)
{
    return read_numeric(find_struct_prop(save_parameter.type, {STR("Exp")}), save_parameter.ptr);
}
}

RC::Unreal::UObject* guild_by_id(const RC::Unreal::FGuid& guild_id);

RC::Unreal::UObject* guild_info_for(const RC::Unreal::FGuid& guild_id);
}
