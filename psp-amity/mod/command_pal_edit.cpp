#include "game_commands.hpp"

#include "command_common.hpp"
#include "game_call.hpp"
#include "reflect.hpp"
#include "snapshots.hpp"

#include <Unreal/CoreUObject/UObject/FStrProperty.hpp>

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt;
using namespace amity_rt::snap;

namespace
{
constexpr int64_t kMaxSlotIndex = 959;
constexpr size_t kMaxNameLength = 128;
constexpr size_t kMaxListLength = 32;

struct NoParams
{
    uint8_t reserved{};
};

struct IntField
{
    const char* key;
    const wchar_t* field;
    int64_t min;
    int64_t max;
};

constexpr IntField kIntFields[] = {
    {"level", STR("Level"), 1, 255},
    {"rank", STR("Rank"), 1, 255},
    {"exp", STR("Exp"), 0, 100000000},
    {"talentHp", STR("Talent_HP"), 0, 255},
    {"talentMelee", STR("Talent_Melee"), 0, 255},
    {"talentShot", STR("Talent_Shot"), 0, 255},
    {"talentDefense", STR("Talent_Defense"), 0, 255},
    {"rankHp", STR("Rank_HP"), 0, 255},
    {"rankAttack", STR("Rank_Attack"), 0, 255},
    {"rankDefense", STR("Rank_Defence"), 0, 255},
    {"rankCraftSpeed", STR("Rank_CraftSpeed"), 0, 255},
    {"friendshipPoint", STR("FriendshipPoint"), 0, 999999},
};

struct FloatField
{
    const char* key;
    const wchar_t* field;
    double min;
    double max;
};

constexpr FloatField kFloatFields[] = {
    {"sanity", STR("SanityValue"), 0.0, 100.0},
    {"stomach", STR("FullStomach"), 0.0, 10000.0},
};

struct BoolField
{
    const char* key;
    const wchar_t* field;
};

constexpr BoolField kBoolFields[] = {
    {"isAwakened", STR("bIsAwakening")},
    {"isLucky", STR("IsRarePal")},
};

nlohmann::json to_json_array(const std::vector<std::string>& values)
{
    nlohmann::json out = nlohmann::json::array();
    for (const std::string& value : values)
    {
        out.push_back(value);
    }
    return out;
}

nlohmann::json read_state(UObject* parameter, const SaveParameterView& save)
{
    nlohmann::json entry = nlohmann::json::object();

    for (const IntField& field : kIntFields)
    {
        entry[field.key] = nullptr;
        if (auto value = read_numeric(find_struct_prop(save.type, {field.field}), save.ptr))
        {
            entry[field.key] = *value;
        }
    }
    for (const FloatField& field : kFloatFields)
    {
        entry[field.key] = nullptr;
        if (auto value = read_float(find_struct_prop(save.type, {field.field}), save.ptr))
        {
            entry[field.key] = *value;
        }
    }
    for (const BoolField& field : kBoolFields)
    {
        entry[field.key] = nullptr;
        if (auto value = read_bool(find_struct_prop(save.type, {field.field}), save.ptr))
        {
            entry[field.key] = *value;
        }
    }

    entry["nickname"] = nullptr;
    if (FString* nickname = value_ptr<FString>(find_struct_prop(save.type, {STR("NickName")}), save.ptr))
    {
        entry["nickname"] = to_utf8(nickname->operator*());
    }

    entry["gender"] = nullptr;
    if (auto gender = read_numeric(find_struct_prop(save.type, {STR("Gender")}), save.ptr))
    {
        if (auto name = gender_name(*gender))
        {
            entry["gender"] = *name;
        }
    }

    entry["hp"] = nullptr;
    if (auto hp = read_fixed_point(find_struct_prop(save.type, {STR("Hp"), STR("HP")}), save.ptr))
    {
        entry["hp"] = *hp;
    }
    entry["maxHp"] = nullptr;
    if (auto max_hp = read_fixed_point(find_struct_prop(save.type, {STR("MaxHP"), STR("MaxHp")}), save.ptr))
    {
        entry["maxHp"] = *max_hp;
    }

    entry["activeSkills"] = to_json_array(read_enum_array(find_struct_prop(save.type, {STR("EquipWaza")}), save.ptr));
    entry["passiveSkills"] =
        to_json_array(read_name_array(find_struct_prop(save.type, {STR("PassiveSkillList")}), save.ptr));
    entry["workSuitability"] =
        read_work_suitability(find_struct_prop(save.type, {STR("GotWorkSuitabilityAddRankList")}), save.ptr);

    if (UFunction* get_level = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:GetLevel")))
    {
        struct IntReturn
        {
            int32_t ReturnValue{};
        } level{};
        parameter->ProcessEvent(get_level, &level);
        entry["level"] = level.ReturnValue;
    }
    return entry;
}

bool call_with_name(UObject* parameter, const wchar_t* path, std::initializer_list<const wchar_t*> aliases, const FName& value)
{
    UFunction* fn = find_function(path);
    if (!fn)
    {
        return false;
    }
    ParamBlock block(fn);
    FName* target = block.valid() ? block.name_at(aliases) : nullptr;
    if (!target)
    {
        return false;
    }
    *target = value;
    block.invoke(parameter);
    return true;
}

bool call_with_enum(UObject* parameter, const wchar_t* path, std::initializer_list<const wchar_t*> aliases, const std::string& enumerator)
{
    UFunction* fn = find_function(path);
    if (!fn)
    {
        return false;
    }
    ParamBlock block(fn);
    if (!block.valid() || !block.set_enum_by_name(aliases, widen(enumerator).c_str()))
    {
        return false;
    }
    block.invoke(parameter);
    return true;
}

nlohmann::json apply_active_skills(UObject* parameter, const nlohmann::json& list)
{
    UFunction* clear_fn = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:ClearEquipWaza"));
    if (!clear_fn)
    {
        return kStepFailed;
    }
    NoParams params{};
    parameter->ProcessEvent(clear_fn, &params);

    for (const auto& entry : list)
    {
        if (!entry.is_string() ||
            !call_with_enum(parameter,
                             STR("/Script/Pal.PalIndividualCharacterParameter:AddEquipWaza"),
                             {STR("WazaID"), STR("WazaId")},
                             entry.get<std::string>()))
        {
            return kStepFailed;
        }
    }
    return kStepOk;
}

nlohmann::json apply_passive_skills(UObject* parameter, const SaveParameterView& save, const nlohmann::json& list)
{
    for (const std::string& name : read_name_array(find_struct_prop(save.type, {STR("PassiveSkillList")}), save.ptr))
    {
        const FName skill(widen(name).c_str(), FNAME_Find);
        if (!skill.IsNone())
        {
            call_with_name(parameter,
                            STR("/Script/Pal.PalIndividualCharacterParameter:RemovePassiveSkill"),
                            {STR("SkillId"), STR("SkillID")},
                            skill);
        }
    }

    for (const auto& entry : list)
    {
        if (!entry.is_string())
        {
            return kStepFailed;
        }
        const FName skill(widen(entry.get<std::string>()).c_str(), FNAME_Find);
        if (skill.IsNone())
        {
            return kStepFailed;
        }
        UFunction* add_fn = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:AddPassiveSkill"));
        if (!add_fn)
        {
            return kStepFailed;
        }
        ParamBlock block(add_fn);
        FName* add = block.valid() ? block.name_at({STR("AddSkill")}) : nullptr;
        if (!add)
        {
            return kStepFailed;
        }
        *add = skill;
        block.invoke(parameter);
    }
    return kStepOk;
}

nlohmann::json apply_work_suitability(UObject* parameter, const nlohmann::json& map)
{
    UFunction* fn = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:SetWorkSuitabilityAddRank"));
    if (!fn)
    {
        return kStepFailed;
    }
    for (auto it = map.begin(); it != map.end(); ++it)
    {
        if (!it.value().is_number_integer())
        {
            return kStepFailed;
        }
        const int64_t rank = it.value().get<int64_t>();
        if (rank < 0 || rank > 10)
        {
            return kStepFailed;
        }
        ParamBlock block(fn);
        if (!block.valid() ||
            !block.set_enum_by_name({STR("WorkSuitability")}, widen(it.key()).c_str()) ||
            !block.set_integral({STR("addRank"), STR("AddRank")}, rank))
        {
            return kStepFailed;
        }
        block.invoke(parameter);
    }
    return kStepOk;
}

// The nickname is an FString inside the save parameter. It is assigned through the property
// system, which deep-copies: the destination frees what it held and takes its own buffer from
// the same allocator, so nothing is left shared with the temporary built here.
nlohmann::json apply_nickname(const SaveParameterView& save, const std::string& nickname)
{
    auto* prop = CastField<FStrProperty>(find_struct_prop(save.type, {STR("NickName"), STR("Nickname")}));
    if (!prop)
    {
        return kStepFailed;
    }
    FString value(widen(nickname).c_str());
    prop->CopyCompleteValue(prop->ContainerPtrToValuePtr<void>(save.ptr), &value);
    return kStepOk;
}

bool requested(const nlohmann::json& args, const char* key)
{
    return args.contains(key) && !args[key].is_null();
}

bool same_set(const nlohmann::json& a, const nlohmann::json& b)
{
    if (!a.is_array() || !b.is_array() || a.size() != b.size())
    {
        return false;
    }
    std::vector<std::string> left, right;
    for (const auto& entry : a)
    {
        left.push_back(entry.is_string() ? entry.get<std::string>() : entry.dump());
    }
    for (const auto& entry : b)
    {
        right.push_back(entry.is_string() ? entry.get<std::string>() : entry.dump());
    }
    std::sort(left.begin(), left.end());
    std::sort(right.begin(), right.end());
    return left == right;
}

bool matches(const nlohmann::json& wanted, const nlohmann::json& observed)
{
    if (observed.is_null())
    {
        return false;
    }
    if (wanted.is_array())
    {
        return same_set(wanted, observed);
    }
    if (wanted.is_object())
    {
        if (!observed.is_object())
        {
            return false;
        }
        for (auto it = wanted.begin(); it != wanted.end(); ++it)
        {
            const auto found = observed.find(it.key());
            const int64_t seen = found == observed.end() ? 0 : found->get<int64_t>();
            if (seen != it.value().get<int64_t>())
            {
                return false;
            }
        }
        return true;
    }
    if (wanted.is_number_float() || observed.is_number_float())
    {
        const double diff = wanted.get<double>() - observed.get<double>();
        return (diff < 0 ? -diff : diff) < 0.01;
    }
    return wanted == observed;
}
}

namespace amity_rt
{
amity::GameResponse command_pal_edit(const std::string& command_id, const nlohmann::json& args)
{
    FGuid player_uid{};
    std::string error{};
    if (!parse_player_uid(args, player_uid, error))
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

    int64_t slot_index = -1;
    if (!instance_id)
    {
        if (!args.contains("slotIndex") || !args["slotIndex"].is_number_integer())
        {
            return amity::GameResponse::fail("validation_failed",
                                              "slotIndex is required and must be an integer");
        }
        slot_index = args["slotIndex"].get<int64_t>();
        if (slot_index < 0 || slot_index > kMaxSlotIndex)
        {
            return amity::GameResponse::fail("validation_failed", "slotIndex is out of range");
        }
    }

    UObject* world_context = any_player_controller();
    if (!world_context)
    {
        return amity::GameResponse::fail("capability_unavailable", "no live world context");
    }

    std::string error_code{}, error_message{};
    UObject* parameter = nullptr;
    if (instance_id)
    {
        parameter = parameter_by_instance_id(world_context, player_uid, *instance_id);
        if (!parameter)
        {
            return amity::GameResponse::fail("validation_failed", "no pal with that instanceId");
        }
    }
    else
    {
        parameter = individual_parameter_by_slot_index(
            world_context, player_uid, static_cast<int32_t>(slot_index), error_code, error_message);
        if (!parameter)
        {
            return amity::GameResponse::fail(error_code, error_message);
        }
    }

    const SaveParameterView save = save_parameter_of(parameter);
    if (!save.ptr || !save.type)
    {
        return amity::GameResponse::fail("capability_unavailable", "pal save parameter unavailable");
    }

    const nlohmann::json before = read_state(parameter, save);

    nlohmann::json wanted = nlohmann::json::object();
    nlohmann::json steps = nlohmann::json::object();

    for (const IntField& field : kIntFields)
    {
        steps[field.key] = kStepSkipped;
        if (!requested(args, field.key))
        {
            continue;
        }
        if (!args[field.key].is_number_integer())
        {
            return amity::GameResponse::fail("validation_failed", std::string(field.key) + " must be an integer");
        }
        const int64_t value = args[field.key].get<int64_t>();
        if (value < field.min || value > field.max)
        {
            return amity::GameResponse::fail("validation_failed", std::string(field.key) + " is out of range");
        }
        wanted[field.key] = value;
        steps[field.key] = write_integral(find_struct_prop(save.type, {field.field}), save.ptr, value) ? kStepOk : kStepFailed;
    }

    for (const FloatField& field : kFloatFields)
    {
        steps[field.key] = kStepSkipped;
        if (!requested(args, field.key))
        {
            continue;
        }
        if (!args[field.key].is_number())
        {
            return amity::GameResponse::fail("validation_failed", std::string(field.key) + " must be a number");
        }
        const double value = args[field.key].get<double>();
        if (value < field.min || value > field.max)
        {
            return amity::GameResponse::fail("validation_failed", std::string(field.key) + " is out of range");
        }
        wanted[field.key] = value;
        steps[field.key] = write_float(find_struct_prop(save.type, {field.field}), save.ptr, value) ? kStepOk : kStepFailed;
    }

    for (const BoolField& field : kBoolFields)
    {
        steps[field.key] = kStepSkipped;
        if (!requested(args, field.key))
        {
            continue;
        }
        if (!args[field.key].is_boolean())
        {
            return amity::GameResponse::fail("validation_failed", std::string(field.key) + " must be a boolean");
        }
        const bool value = args[field.key].get<bool>();
        wanted[field.key] = value;
        auto* prop = CastField<FBoolProperty>(find_struct_prop(save.type, {field.field}));
        if (prop)
        {
            prop->SetPropertyValueInContainer(save.ptr, value);
        }
        steps[field.key] = prop ? kStepOk : kStepFailed;
    }

    steps["hp"] = kStepSkipped;
    if (requested(args, "hp"))
    {
        if (!args["hp"].is_number_integer() || args["hp"].get<int64_t>() < 0)
        {
            return amity::GameResponse::fail("validation_failed", "hp must be a non-negative integer");
        }
        const int64_t value = args["hp"].get<int64_t>();
        wanted["hp"] = value;
        steps["hp"] = write_fixed_point(find_struct_prop(save.type, {STR("Hp"), STR("HP")}), save.ptr, value) ? kStepOk
                                                                                                              : kStepFailed;
    }

    steps["gender"] = kStepSkipped;
    if (requested(args, "gender"))
    {
        if (!args["gender"].is_string())
        {
            return amity::GameResponse::fail("validation_failed", "gender must be a string");
        }
        std::string gender = args["gender"].get<std::string>();
        for (char& c : gender)
        {
            c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
        }
        wanted["gender"] = gender;
        const std::wstring enumerator = gender == "male" ? STR("Male") : gender == "female" ? STR("Female") : STR("None");
        steps["gender"] =
            write_enum_by_name(find_struct_prop(save.type, {STR("Gender")}), save.ptr, enumerator.c_str()) ? kStepOk
                                                                                                           : kStepFailed;
    }

    steps["nickname"] = kStepSkipped;
    if (requested(args, "nickname"))
    {
        if (!args["nickname"].is_string() || args["nickname"].get<std::string>().size() > kMaxNameLength)
        {
            return amity::GameResponse::fail("validation_failed", "nickname must be a string of at most 128 characters");
        }
        wanted["nickname"] = args["nickname"];
        steps["nickname"] = apply_nickname(save, args["nickname"].get<std::string>());
    }

    steps["activeSkills"] = kStepSkipped;
    if (requested(args, "activeSkills") && args["activeSkills"].is_array())
    {
        if (args["activeSkills"].size() > kMaxListLength)
        {
            return amity::GameResponse::fail("validation_failed", "activeSkills is too long");
        }
        wanted["activeSkills"] = args["activeSkills"];
        steps["activeSkills"] = apply_active_skills(parameter, args["activeSkills"]);
    }

    steps["passiveSkills"] = kStepSkipped;
    if (requested(args, "passiveSkills") && args["passiveSkills"].is_array())
    {
        if (args["passiveSkills"].size() > kMaxListLength)
        {
            return amity::GameResponse::fail("validation_failed", "passiveSkills is too long");
        }
        wanted["passiveSkills"] = args["passiveSkills"];
        steps["passiveSkills"] = apply_passive_skills(parameter, save, args["passiveSkills"]);
    }

    steps["workSuitability"] = kStepSkipped;
    if (requested(args, "workSuitability") && args["workSuitability"].is_object())
    {
        wanted["workSuitability"] = args["workSuitability"];
        steps["workSuitability"] = apply_work_suitability(parameter, args["workSuitability"]);
    }

    steps["resync"] = kStepSkipped;
    if (!requested(args, "hp"))
    {
        if (UFunction* recover = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:FullRecoveryHP")))
        {
            NoParams params{};
            parameter->ProcessEvent(recover, &params);
            steps["resync"] = kStepOk;
        }
    }

    const nlohmann::json after = read_state(parameter, save);

    nlohmann::json unconfirmed = nlohmann::json::array();
    for (auto it = wanted.begin(); it != wanted.end(); ++it)
    {
        const auto observed = after.find(it.key());
        if (observed == after.end() || !matches(it.value(), *observed))
        {
            unconfirmed.push_back(it.key());
        }
    }

    nlohmann::json data = nlohmann::json::object();
    data["steps"] = std::move(steps);
    data["before"] = before;
    data["after"] = after;
    data["unconfirmed"] = unconfirmed;

    return command_result(command_id, kOpPalEdit, !wanted.empty(), !wanted.empty() && unconfirmed.empty(), true, std::move(data));
}
}
