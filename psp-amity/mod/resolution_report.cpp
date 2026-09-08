#include "resolution_report.hpp"
#include "reflect.hpp"
#include "signature_check.hpp"

#include <initializer_list>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

#include <DynamicOutput/DynamicOutput.hpp>
#include <Unreal/FField.hpp>

using namespace RC;
using namespace RC::Unreal;

namespace
{
enum class ItemState
{
    Deferred,
    Ok,
    Missing,
};

bool is_terminal(ItemState state)
{
    return state == ItemState::Ok || state == ItemState::Missing;
}

const wchar_t* state_text(ItemState state)
{
    switch (state)
    {
    case ItemState::Ok:
        return STR("ok");
    case ItemState::Missing:
        return STR("MISSING");
    default:
        return STR("deferred");
    }
}

struct ReportState
{
    std::unordered_map<std::wstring, ItemState> items{};
    bool complete{false};
};

ReportState& report_state()
{
    static ReportState state{};
    return state;
}

void update_item(ReportState& state, const wchar_t* name, ItemState new_state)
{
    auto it = state.items.find(name);
    if (it != state.items.end())
    {
        if (is_terminal(it->second) || it->second == new_state)
        {
            return;
        }
    }
    Output::send<LogLevel::Verbose>(STR("[PSPAmity] resolve {} -> {}\n"), name, state_text(new_state));
    state.items[name] = new_state;
}

bool is_settled(const ReportState& state, const wchar_t* name)
{
    auto it = state.items.find(name);
    return it != state.items.end() && is_terminal(it->second);
}

ItemState class_state(const wchar_t* full_path)
{
    return amity_rt::find_class(full_path) != nullptr ? ItemState::Ok : ItemState::Missing;
}

ItemState enum_state(const wchar_t* full_path)
{
    return amity_rt::find_enum(full_path) != nullptr ? ItemState::Ok : ItemState::Missing;
}

ItemState struct_state(const wchar_t* full_path)
{
    return amity_rt::find_script_struct(full_path) != nullptr ? ItemState::Ok : ItemState::Missing;
}

ItemState prop_state(UObject* instance, std::initializer_list<const wchar_t*> aliases)
{
    if (!instance)
    {
        return ItemState::Deferred;
    }
    return amity_rt::find_prop(instance, aliases) != nullptr ? ItemState::Ok : ItemState::Missing;
}

ItemState struct_member_state(UStruct* strct, bool have_instance, std::initializer_list<const wchar_t*> aliases)
{
    if (!have_instance)
    {
        return ItemState::Deferred;
    }
    return amity_rt::find_struct_prop(strct, aliases) != nullptr ? ItemState::Ok : ItemState::Missing;
}

UObject* first_instance_of(const wchar_t* class_name)
{
    std::vector<UObject*> instances{};
    UObjectGlobals::FindAllOf(class_name, instances);
    for (UObject* instance : instances)
    {
        if (instance)
        {
            return instance;
        }
    }
    return nullptr;
}

std::wstring function_param_list(UFunction* fn)
{
    std::wstring params{};
    bool first = true;
    for (FProperty* prop : TFieldRange<FProperty>(fn, EFieldIterationFlags::None))
    {
        if (!prop)
        {
            continue;
        }
        if (!first)
        {
            params += STR(", ");
        }
        first = false;
        params += prop->GetName();
        params += STR(":");
        params += prop->GetClass().GetName();
        params += STR("(");
        params += std::to_wstring(prop->GetSize());
        params += STR(")");
    }
    if (first)
    {
        params = STR("<none>");
    }
    return params;
}

std::wstring short_function_name(const wchar_t* full_path)
{
    const std::wstring path = full_path;
    const size_t dot = path.rfind(L'.');
    return dot == std::wstring::npos ? path : path.substr(dot + 1);
}

void update_function_item(ReportState& state, const wchar_t* full_path)
{
    const std::wstring name = short_function_name(full_path);
    if (is_settled(state, name.c_str()))
    {
        return;
    }
    UFunction* fn = amity_rt::find_function(full_path);
    update_item(state, name.c_str(), fn != nullptr ? ItemState::Ok : ItemState::Missing);
    if (fn)
    {
        Output::send<LogLevel::Verbose>(STR("[PSPAmity] params {}: {}\n"), name, function_param_list(fn));
    }
}

void update_signature_table_functions(ReportState& state)
{
    std::unordered_set<std::wstring> seen{};
    auto walk = [&](const amity_sig::OpSpec* specs, std::size_t count) {
        for (std::size_t i = 0; i < count; ++i)
        {
            for (std::size_t f = 0; f < specs[i].function_count; ++f)
            {
                const wchar_t* path = specs[i].functions[f].path;
                if (seen.insert(path).second)
                {
                    update_function_item(state, path);
                }
            }
        }
    };
    // The count is an out-parameter, so it must be settled before it is passed: as a second
    // argument to the same call its value is read before the call that fills it.
    std::size_t write_count = 0;
    const amity_sig::OpSpec* writes = amity_sig::op_specs(write_count);
    walk(writes, write_count);
    std::size_t read_count = 0;
    const amity_sig::OpSpec* reads = amity_sig::read_op_specs(read_count);
    walk(reads, read_count);
}

constexpr const wchar_t* kSnapshotFunctions[] = {
    STR("/Script/Engine.KismetSystemLibrary:IsDedicatedServer"),
    STR("/Script/Engine.KismetSystemLibrary:IsStandalone"),
    STR("/Script/Engine.KismetSystemLibrary:IsServer"),
    STR("/Script/Engine.Actor:K2_GetActorLocation"),
    STR("/Script/Engine.Actor:K2_GetActorRotation"),
    STR("/Script/Pal.PalPlayerDataPalStorage:GetPageNum"),
    STR("/Script/Pal.PalPlayerDataPalStorage:GetSlotsInPage"),
    STR("/Script/Pal.PalIndividualCharacterSlot:GetSlotIndex"),
    STR("/Script/Pal.PalIndividualCharacterParameter:IsDead"),
};

constexpr const wchar_t* kSaveParameterMembers[] = {
    L"CharacterID", L"NickName",      L"Level",         L"Exp",           L"Rank",        L"Talent_HP",
    L"Talent_Shot", L"Talent_Defense", L"Rank_HP",       L"Rank_Attack",   L"Rank_CraftSpeed",
    L"PassiveSkillList", L"Gender",    L"IsRarePal",     L"OwnerPlayerUId", L"EquipWaza", L"MasteredWaza",
    L"Hp",          L"SanityValue",   L"WorkerSick",    L"HungerType",    L"FullStomach", L"SlotId",
    L"FriendshipPoint", L"GotWorkSuitabilityAddRankList",
};

struct InstanceProps
{
    const wchar_t* class_name;
    std::initializer_list<const wchar_t*> members;
};

const InstanceProps kInstanceProps[] = {
    {STR("PalPlayerState"), {STR("PlayerUId"), STR("PlayerNamePrivate"), STR("IndividualHandleId"), STR("GuildBelongTo")}},
    {STR("PalItemContainer"), {STR("ItemSlotArray"), STR("ID"), STR("BelongInfo")}},
    {STR("PalItemSlot"), {STR("SlotIndex"), STR("ItemId"), STR("StackCount"), STR("DynamicItemData")}},
    {STR("PalGroupGuildBase"),
     {STR("ID"), STR("GuildName"), STR("GroupName"), STR("BaseCampLevel"), STR("AdminPlayerUId"),
      STR("PlayerInfoRepInfoArray"), STR("BaseCampIds"), STR("MapObjectInstanceIds_BaseCampPoint"),
      STR("ItemStorage"), STR("Lab")}},
    {STR("PalGuildInfo"), {STR("GroupId"), STR("PlayerUIds")}},
    {STR("PalBaseCampModel"), {STR("ID"), STR("GroupIdBelongTo"), STR("WorkerDirector"), STR("Level_InGuildProperty")}},
    {STR("PalMapObjectItemContainerModule"), {STR("TargetContainer")}},
    {STR("PalIndividualCharacterContainer"), {STR("SlotArray"), STR("ID")}},
};

constexpr const wchar_t* kClasses[] = {
    STR("/Script/Pal.PalIndividualCharacterParameter"),
    STR("/Script/Pal.PalPlayerState"),
    STR("/Script/Pal.PalItemContainer"),
    STR("/Script/Pal.PalItemSlot"),
    STR("/Script/Pal.PalDynamicItemDataBase"),
    STR("/Script/Pal.PalDynamicWeaponItemDataBase"),
    STR("/Script/Pal.PalDynamicArmorItemDataBase"),
    STR("/Script/Pal.PalDynamicPalEggItemDataBase"),
    STR("/Script/Pal.PalOtomoHolderComponentBase"),
};

constexpr const wchar_t* kStructs[] = {
    STR("/Script/Pal.PalContainerId"),
    STR("/Script/Pal.PalItemSlotId"),
    STR("/Script/Pal.PalItemSlotIdAndNum"),
    STR("/Script/Pal.PalCharacterSlotId"),
    STR("/Script/Pal.PalInstanceID"),
    STR("/Script/Pal.PalIndividualCharacterSaveParameter"),
    STR("/Script/Pal.PalGuildPlayerInfo"),
};

constexpr const wchar_t* kEnums[] = {
    STR("/Script/Pal.EPalPlayerInventoryType"),
    STR("/Script/Pal.EPalItemOperationResult"),
    STR("/Script/Pal.EPalStatusPhysicalHealthType"),
};

std::wstring short_type_name(const wchar_t* full_path)
{
    return short_function_name(full_path);
}
}

namespace amity_rt
{
bool update_resolution_report()
{
    ReportState& state = report_state();
    if (state.complete)
    {
        return true;
    }

    update_signature_table_functions(state);
    for (const wchar_t* path : kSnapshotFunctions)
    {
        update_function_item(state, path);
    }
    for (const wchar_t* path : kClasses)
    {
        update_item(state, short_type_name(path).c_str(), class_state(path));
    }
    for (const wchar_t* path : kStructs)
    {
        update_item(state, short_type_name(path).c_str(), struct_state(path));
    }
    for (const wchar_t* path : kEnums)
    {
        update_item(state, short_type_name(path).c_str(), enum_state(path));
    }

    for (const InstanceProps& props : kInstanceProps)
    {
        UObject* instance = first_instance_of(props.class_name);
        for (const wchar_t* member : props.members)
        {
            const std::wstring name = std::wstring(props.class_name) + L"." + member;
            update_item(state, name.c_str(), prop_state(instance, {member}));
        }
    }

    UObject* character_parameter = first_instance_of(STR("PalIndividualCharacterParameter"));
    FProperty* save_parameter_prop = character_parameter ? find_prop(character_parameter, {STR("SaveParameter")}) : nullptr;
    const ItemState save_parameter_state = !character_parameter    ? ItemState::Deferred
                                            : save_parameter_prop ? ItemState::Ok
                                                                   : ItemState::Missing;
    update_item(state, STR("PalIndividualCharacterParameter.SaveParameter"), save_parameter_state);

    UStruct* save_parameter_struct = nullptr;
    if (auto* struct_prop = CastField<FStructProperty>(save_parameter_prop))
    {
        save_parameter_struct = struct_prop->GetStruct();
    }
    const bool have_character_parameter = character_parameter != nullptr;
    for (const wchar_t* member : kSaveParameterMembers)
    {
        const std::wstring name = std::wstring(L"SaveParameter.") + member;
        update_item(state, name.c_str(), struct_member_state(save_parameter_struct, have_character_parameter, {member}));
    }
    update_item(state,
                STR("SaveParameter.Rank_Defence"),
                struct_member_state(save_parameter_struct, have_character_parameter, {L"Rank_Defence", L"Rank_Defense"}));

    bool any_deferred = false;
    int ok_count = 0;
    int missing_count = 0;
    for (const auto& [name, item_state] : state.items)
    {
        switch (item_state)
        {
        case ItemState::Deferred:
            any_deferred = true;
            break;
        case ItemState::Ok:
            ++ok_count;
            break;
        case ItemState::Missing:
            ++missing_count;
            break;
        }
    }

    if (!any_deferred)
    {
        Output::send<LogLevel::Verbose>(STR("[PSPAmity] resolution report complete: {} ok, {} MISSING\n"), ok_count, missing_count);
        state.complete = true;
    }
    return state.complete;
}
}
