#include "game_commands.hpp"

#include "command_common.hpp"
#include "game_call.hpp"
#include "reflect.hpp"
#include "snapshots.hpp"

#include <cstdint>
#include <optional>
#include <string>
#include <utility>

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt;
using namespace amity_rt::snap;

namespace
{
struct IntField
{
    const char* key;
    const wchar_t* field;
    int64_t min;
    int64_t max;
};

constexpr IntField kIntFields[] = {
    {"level", STR("Level"), 1, 100},
    {"exp", STR("Exp"), 0, 100000000},
};

bool requested(const nlohmann::json& args, const char* key)
{
    return args.contains(key) && !args[key].is_null();
}

nlohmann::json read_state(const SaveParameterView& save)
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
    return entry;
}

bool matches_request(const nlohmann::json& args, const nlohmann::json& state)
{
    for (const IntField& field : kIntFields)
    {
        if (!requested(args, field.key))
        {
            continue;
        }
        if (!state.contains(field.key) || state[field.key].is_null())
        {
            return false;
        }
        if (state[field.key].get<int64_t>() != args[field.key].get<int64_t>())
        {
            return false;
        }
    }
    return true;
}
}

namespace amity_rt
{
amity::GameResponse command_player_edit(const std::string& command_id, const nlohmann::json& args)
{
    FGuid player_uid{};
    std::string error{};
    if (!parse_player_uid(args, player_uid, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }

    bool any_requested = false;
    for (const IntField& field : kIntFields)
    {
        if (!requested(args, field.key))
        {
            continue;
        }
        any_requested = true;
        if (!args[field.key].is_number_integer())
        {
            return amity::GameResponse::fail("validation_failed",
                                             std::string(field.key) + " must be an integer");
        }
        const int64_t value = args[field.key].get<int64_t>();
        if (value < field.min || value > field.max)
        {
            return amity::GameResponse::fail("validation_failed",
                                             std::string(field.key) + " is out of range");
        }
    }
    if (!any_requested)
    {
        return amity::GameResponse::fail("validation_failed", "no editable field was given");
    }

    LivePlayer live{};
    if (auto failure = require_live_player(player_uid, live))
    {
        return *failure;
    }
    UObject* world_context = live.world_context;
    UObject* player_state = live.player_state;

    UFunction* get_char_manager_fn = find_function(STR("/Script/Pal.PalUtility:GetCharacterManager"));
    UFunction* get_individual_param_fn =
        find_function(STR("/Script/Pal.PalCharacterManager:GetIndividualCharacterParameter"));
    if (!get_char_manager_fn || !get_individual_param_fn)
    {
        return amity::GameResponse::fail("capability_unavailable", "player parameter functions unresolved");
    }

    const SaveParameterView save =
        player_save_parameter(world_context, player_state, get_char_manager_fn, get_individual_param_fn);
    if (!save.ptr || !save.type)
    {
        return amity::GameResponse::fail("game_error", "player save parameter unavailable");
    }

    nlohmann::json steps = nlohmann::json::object();
    bool any_step_ran = false;
    for (const IntField& field : kIntFields)
    {
        steps[field.key] = kStepSkipped;
        if (!requested(args, field.key))
        {
            continue;
        }
        const int64_t value = args[field.key].get<int64_t>();
        const bool ok = write_integral(find_struct_prop(save.type, {field.field}), save.ptr, value);
        steps[field.key] = ok ? kStepOk : kStepFailed;
        any_step_ran = any_step_ran || ok;
    }

    const nlohmann::json state = read_state(save);

    nlohmann::json data = nlohmann::json::object();
    data["steps"] = steps;
    data["player"] = state;

    return command_result(command_id, kOpPlayerEdit, any_step_ran, matches_request(args, state), true, std::move(data));
}
}
