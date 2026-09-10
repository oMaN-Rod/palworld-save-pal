#pragma once

#include "reflect.hpp"
#include "snapshots.hpp"

#include <Unreal/FField.hpp>
#include <Unreal/Property/FEnumProperty.hpp>

#include <cstdint>
#include <initializer_list>
#include <string>
#include <utility>
#include <vector>

namespace amity_rt
{
using namespace RC;
using namespace RC::Unreal;

// A parm block is only ever as wide as the function says it is; anything past that is a
// function we have no business calling with a stack of this shape.
inline constexpr int32_t kMaxParamBlockSize = 4096;

class ParamBlock
{
public:
    explicit ParamBlock(UFunction* fn)
    {
        if (!fn)
        {
            return;
        }
        const int32_t size = fn->GetPropertiesSize();
        if (size <= 0 || size > kMaxParamBlockSize)
        {
            return;
        }
        buffer_.assign(static_cast<size_t>(size), 0);
        for (FProperty* prop : TFieldRange<FProperty>(fn, EFieldIterationFlags::None))
        {
            if (prop && prop->HasAnyPropertyFlags(CPF_Parm))
            {
                prop->InitializeValue_InContainer(buffer_.data());
                parms_.push_back(prop);
            }
        }
        fn_ = fn;
    }

    ~ParamBlock()
    {
        for (auto it = parms_.rbegin(); it != parms_.rend(); ++it)
        {
            (*it)->DestroyValue_InContainer(buffer_.data());
        }
    }

    ParamBlock(const ParamBlock&) = delete;
    ParamBlock& operator=(const ParamBlock&) = delete;

    bool valid() const { return fn_ != nullptr; }

    FProperty* param(std::initializer_list<const wchar_t*> aliases) const
    {
        FProperty* prop = find_struct_prop(fn_, aliases);
        return (prop && prop->HasAnyPropertyFlags(CPF_Parm)) ? prop : nullptr;
    }

    // A pointer TO the parameter's value. Never hand this to value_ptr() or write_integral():
    // those take a CONTAINER and offset it themselves, so passing an already-offset pointer
    // writes at double the offset, into whatever parameter comes next. Use the checked
    // accessors below for leaf writes; this stays for whole-struct work, which takes a value.
    void* value_of(FProperty* prop)
    {
        return prop ? prop->ContainerPtrToValuePtr<void>(buffer_.data()) : nullptr;
    }

    FName* name_at(std::initializer_list<const wchar_t*> aliases)
    {
        auto* prop = CastField<FNameProperty>(param(aliases));
        return prop ? static_cast<FName*>(value_of(prop)) : nullptr;
    }

    FGuid* guid_at(std::initializer_list<const wchar_t*> aliases)
    {
        auto* prop = CastField<FStructProperty>(param(aliases));
        UStruct* type = prop ? prop->GetStruct() : nullptr;
        if (!type || type->GetName() != STR("Guid") || prop->GetSize() != static_cast<int32_t>(sizeof(FGuid)))
        {
            return nullptr;
        }
        return static_cast<FGuid*>(value_of(prop));
    }

    bool set_integral(std::initializer_list<const wchar_t*> aliases, int64_t value)
    {
        FProperty* prop = param(aliases);
        if (auto* enum_prop = CastField<FEnumProperty>(prop))
        {
            FNumericProperty* underlying = enum_prop->GetUnderlyingProperty();
            if (!underlying)
            {
                return false;
            }
            underlying->SetIntPropertyValue(value_of(prop), value);
            return true;
        }
        if (auto* numeric = CastField<FNumericProperty>(prop); numeric && !numeric->IsFloatingPoint())
        {
            numeric->SetIntPropertyValue(value_of(prop), value);
            return true;
        }
        return false;
    }

    bool set_enum_by_name(std::initializer_list<const wchar_t*> aliases, const wchar_t* enumerator)
    {
        auto* enum_prop = CastField<FEnumProperty>(param(aliases));
        UEnum* enum_type = enum_prop ? enum_prop->GetEnum().Get() : nullptr;
        if (!enum_type || !enumerator)
        {
            return false;
        }
        std::vector<std::pair<FName, int64_t>> names{};
        enum_type->GetEnumNamesAsVector(names);
        for (const auto& [name, value] : names)
        {
            const std::wstring entry = name.ToString();
            const size_t sep = entry.rfind(L"::");
            const bool matches = entry == enumerator ||
                                  (sep != std::wstring::npos &&
                                   entry.compare(sep + 2, std::wstring::npos, enumerator) == 0);
            if (matches)
            {
                FNumericProperty* underlying = enum_prop->GetUnderlyingProperty();
                if (!underlying)
                {
                    return false;
                }
                underlying->SetIntPropertyValue(value_of(enum_prop), value);
                return true;
            }
        }
        return false;
    }

    bool bool_at(std::initializer_list<const wchar_t*> aliases)
    {
        auto* prop = CastField<FBoolProperty>(param(aliases));
        return prop && prop->GetPropertyValue(value_of(prop));
    }

    void invoke(UObject* target) { target->ProcessEvent(fn_, buffer_.data()); }

private:
    UFunction* fn_{};
    std::vector<uint8_t> buffer_{};
    std::vector<FProperty*> parms_{};
};

inline FStructProperty* struct_param(const ParamBlock& block, std::initializer_list<const wchar_t*> aliases)
{
    return CastField<FStructProperty>(block.param(aliases));
}

inline bool write_slot_id(UStruct* type, void* value, const FGuid& container_id, int32_t slot_index)
{
    if (!type || !value)
    {
        return false;
    }

    auto* container_prop = CastField<FStructProperty>(find_struct_prop(type, {STR("ContainerId"), STR("ContainerID")}));
    if (!container_prop)
    {
        return false;
    }
    void* container_value = container_prop->ContainerPtrToValuePtr<void>(value);
    auto* guid_prop = CastField<FStructProperty>(find_struct_prop(container_prop->GetStruct(), {STR("ID"), STR("Id")}));
    UStruct* guid_type = guid_prop ? guid_prop->GetStruct() : nullptr;
    if (!guid_type || guid_type->GetName() != STR("Guid") || guid_prop->GetSize() != static_cast<int32_t>(sizeof(FGuid)))
    {
        return false;
    }
    FGuid* guid = snap::value_ptr<FGuid>(guid_prop, container_value);
    if (!guid)
    {
        return false;
    }
    *guid = container_id;

    return snap::write_integral(find_struct_prop(type, {STR("SlotIndex"), STR("Index")}), value, slot_index);
}

inline bool write_slot_id_and_num(UStruct* type, void* value, const FGuid& container_id, int32_t slot_index, int64_t num)
{
    if (!type || !value)
    {
        return false;
    }
    auto* nested = CastField<FStructProperty>(find_struct_prop(type, {STR("SlotId"), STR("SlotID"), STR("ItemSlotId")}));
    const bool id_written = nested
        ? write_slot_id(nested->GetStruct(), nested->ContainerPtrToValuePtr<void>(value), container_id, slot_index)
        : write_slot_id(type, value, container_id, slot_index);
    if (!id_written)
    {
        return false;
    }
    return snap::write_integral(find_struct_prop(type, {STR("Num"), STR("Count"), STR("StackCount")}), value, num);
}

inline bool write_enum_by_name(FProperty* prop, void* container, const wchar_t* enumerator)
{
    UEnum* enum_type = nullptr;
    if (auto* enum_prop = CastField<FEnumProperty>(prop))
    {
        enum_type = enum_prop->GetEnum().Get();
    }
    else if (auto* numeric = CastField<FNumericProperty>(prop))
    {
        enum_type = numeric->GetIntPropertyEnum();
    }
    if (!enum_type || !container || !enumerator)
    {
        return false;
    }
    std::vector<std::pair<FName, int64_t>> names{};
    enum_type->GetEnumNamesAsVector(names);
    for (const auto& [name, value] : names)
    {
        const std::wstring entry = name.ToString();
        const size_t sep = entry.rfind(L"::");
        const bool matches =
            entry == enumerator || (sep != std::wstring::npos && entry.compare(sep + 2, std::wstring::npos, enumerator) == 0);
        if (matches)
        {
            return snap::write_integral(prop, container, value);
        }
    }
    return false;
}

inline void fill_request_id(UStruct* type, void* value)
{
    static uint32_t sequence = 0;
    if (!type || !value)
    {
        return;
    }
    auto* guid_prop = CastField<FStructProperty>(find_struct_prop(type, {STR("ID"), STR("Id"), STR("RequestId")}));
    UStruct* guid_type = guid_prop ? guid_prop->GetStruct() : nullptr;
    if (!guid_type || guid_type->GetName() != STR("Guid"))
    {
        return;
    }
    if (FGuid* guid = snap::value_ptr<FGuid>(guid_prop, value))
    {
        ++sequence;
        guid->A = 0x50535053;
        guid->B = sequence;
        guid->C = 0;
        guid->D = 0;
    }
}

UObject* network_transmitter(UObject* pawn);
UObject* item_network_component(UObject* pawn);
UObject* character_container_component(UObject* pawn);

UObject* otomo_holder(UObject* player_state);

UObject* player_pawn(UObject* player_state, const char*& reason);

bool write_instance_id(UStruct* type, void* value, const FGuid& player_uid, const FGuid& instance_id);

UObject* object_return_of(ParamBlock& block);

UObject* character_manager(UObject* world_context);

UObject* parameter_by_id(UObject* manager, const FGuid& player_uid, const FGuid& instance_id);

UObject* party_parameter_at(UObject* holder, int32_t seat);
std::string instance_id_of_parameter(UObject* parameter);

UObject* parameter_by_instance_id(UObject* world_context, const FGuid& player_uid, const FGuid& instance_id);

UObject* base_camp_container(const FGuid& base_id, const char*& reason);

UObject* base_container_slot(UObject* container, int32_t slot_index);

nlohmann::json revive_parameter(UObject* parameter);
bool all_steps_ok(const nlohmann::json& steps);
}
