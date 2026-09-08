#include "game_commands.hpp"

#include "command_common.hpp"
#include "game_call.hpp"
#include "reflect.hpp"
#include "snapshots.hpp"

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt;
using namespace amity_rt::snap;

namespace
{
constexpr int64_t kMinBaseCampLevel = 1;
constexpr int64_t kMaxBaseCampLevel = 50;

std::vector<UObject*> base_camps_of(const FGuid& guild_id)
{
    std::vector<UObject*> bases{};
    std::vector<UObject*> models{};
    UObjectGlobals::FindAllOf(STR("PalBaseCampModel"), models);
    for (UObject* model : models)
    {
        if (!model)
        {
            continue;
        }
        FGuid* group_id = value_ptr<FGuid>(find_prop(model, {STR("GroupIdBelongTo")}), model);
        if (group_id && *group_id == guild_id)
        {
            bases.push_back(model);
        }
    }
    return bases;
}

uint8_t* member_row(UObject* guild, const FGuid& member_uid, UStruct*& row_type)
{
    auto* fast_struct = CastField<FStructProperty>(find_prop(guild, {STR("PlayerInfoRepInfoArray")}));
    if (!fast_struct || !fast_struct->GetStruct())
    {
        return nullptr;
    }
    void* fast_ptr = fast_struct->ContainerPtrToValuePtr<void>(guild);
    auto* items = CastField<FArrayProperty>(find_struct_prop(fast_struct->GetStruct(), {STR("Items")}));
    auto* row = items ? CastField<FStructProperty>(items->GetInner()) : nullptr;
    row_type = row ? row->GetStruct() : nullptr;
    if (!items || !row_type)
    {
        return nullptr;
    }

    FScriptArrayHelper helper(items, items->ContainerPtrToValuePtr<void>(fast_ptr));
    for (int32_t i = 0; i < helper.Num(); ++i)
    {
        uint8_t* element = helper.GetRawPtr(i);
        if (!element)
        {
            continue;
        }
        FGuid* uid = value_ptr<FGuid>(find_struct_prop(row_type, {STR("PlayerUId")}), element);
        if (uid && *uid == member_uid)
        {
            return element;
        }
    }
    return nullptr;
}

nlohmann::json read_role(UStruct* row_type, uint8_t* element)
{
    auto* info_prop = CastField<FStructProperty>(find_struct_prop(row_type, {STR("PlayerInfo")}));
    UStruct* info_type = info_prop ? info_prop->GetStruct() : nullptr;
    if (!info_type)
    {
        return nullptr;
    }
    void* info = info_prop->ContainerPtrToValuePtr<void>(element);
    auto* enum_prop = CastField<FEnumProperty>(find_struct_prop(info_type, {STR("Role")}));
    FNumericProperty* underlying = enum_prop ? enum_prop->GetUnderlyingProperty() : nullptr;
    UEnum* enum_type = enum_prop ? enum_prop->GetEnum() : nullptr;
    if (!underlying || !enum_type)
    {
        return nullptr;
    }
    const int64_t value = underlying->GetSignedIntPropertyValue(enum_prop->ContainerPtrToValuePtr<void>(info));
    return bare_enumerator(to_utf8(enum_type->GetNameByValue(value).ToString()));
}

nlohmann::json read_state(UObject* guild, const std::vector<UObject*>& bases)
{
    nlohmann::json entry = nlohmann::json::object();
    entry["baseCampLevel"] = nullptr;
    if (auto level = read_numeric(find_prop(guild, {STR("BaseCampLevel")}), guild))
    {
        entry["baseCampLevel"] = *level;
    }
    nlohmann::json base_levels = nlohmann::json::array();
    for (UObject* base : bases)
    {
        if (auto level = read_numeric(find_prop(base, {STR("Level_InGuildProperty")}), base))
        {
            base_levels.push_back(*level);
        }
        else
        {
            base_levels.push_back(nullptr);
        }
    }
    entry["baseLevels"] = std::move(base_levels);
    return entry;
}

bool matches_request(int64_t wanted, const nlohmann::json& state)
{
    return !state["baseCampLevel"].is_null() && state["baseCampLevel"].get<int64_t>() == wanted;
}
}

namespace amity_rt
{
amity::GameResponse command_guild_edit(const std::string& command_id, const nlohmann::json& args)
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
    if (!args.contains("baseCampLevel") || args["baseCampLevel"].is_null())
    {
        return amity::GameResponse::fail("validation_failed", "no editable field was given");
    }
    if (!args["baseCampLevel"].is_number_integer())
    {
        return amity::GameResponse::fail("validation_failed", "baseCampLevel must be an integer");
    }
    const int64_t wanted = args["baseCampLevel"].get<int64_t>();
    if (wanted < kMinBaseCampLevel || wanted > kMaxBaseCampLevel)
    {
        return amity::GameResponse::fail("validation_failed", "baseCampLevel is out of range");
    }

    UObject* guild = guild_by_id(*guild_id);
    if (!guild)
    {
        return amity::GameResponse::fail("validation_failed", "unknown guildId");
    }

    const std::vector<UObject*> bases = base_camps_of(*guild_id);

    nlohmann::json steps = nlohmann::json::object();

    const bool guild_ok = write_integral(find_prop(guild, {STR("BaseCampLevel")}), guild, wanted);
    steps["guild"] = guild_ok ? kStepOk : kStepFailed;
    const bool any_step_ran = guild_ok;

    const nlohmann::json state = read_state(guild, bases);

    nlohmann::json data = nlohmann::json::object();
    data["steps"] = std::move(steps);
    data["guild"] = state;

    return command_result(command_id, kOpGuildEdit, any_step_ran, matches_request(wanted, state), true, std::move(data));
}
}

namespace amity_rt
{
// A member's role is an enum inside a struct inside the guild's replicated roster array. The
// row is written in place -- no element is added or removed -- so the array never resizes,
// which matters because this UE4SS build cannot link TArray's growth path at all.
amity::GameResponse command_guild_set_role(const std::string& command_id, const nlohmann::json& args)
{
    if (!args.contains("guildId") || !args["guildId"].is_string())
    {
        return amity::GameResponse::fail("validation_failed", "guildId is required");
    }
    if (!args.contains("memberUid") || !args["memberUid"].is_string())
    {
        return amity::GameResponse::fail("validation_failed", "memberUid is required");
    }
    if (!args.contains("role") || !args["role"].is_string())
    {
        return amity::GameResponse::fail("validation_failed", "role is required");
    }
    const std::optional<FGuid> guild_id = parse_guid(args["guildId"].get<std::string>());
    const std::optional<FGuid> member_uid = parse_guid(args["memberUid"].get<std::string>());
    if (!guild_id || !member_uid)
    {
        return amity::GameResponse::fail("validation_failed", "guildId and memberUid must be GUIDs");
    }

    UObject* guild = guild_by_id(*guild_id);
    if (!guild)
    {
        return amity::GameResponse::fail("validation_failed", "unknown guildId");
    }

    UStruct* row_type = nullptr;
    uint8_t* element = member_row(guild, *member_uid, row_type);
    if (!element || !row_type)
    {
        return amity::GameResponse::fail("validation_failed", "that player is not in this guild");
    }

    auto* info_prop = CastField<FStructProperty>(find_struct_prop(row_type, {STR("PlayerInfo")}));
    UStruct* info_type = info_prop ? info_prop->GetStruct() : nullptr;
    if (!info_type)
    {
        return amity::GameResponse::fail("game_error", "member row has no player info");
    }
    void* info = info_prop->ContainerPtrToValuePtr<void>(element);

    const std::string role = args["role"].get<std::string>();
    const std::wstring enumerator = widen(role);
    const bool ok = write_enum_by_name(find_struct_prop(info_type, {STR("Role")}), info, enumerator.c_str());

    const nlohmann::json observed = read_role(row_type, element);

    nlohmann::json data = nlohmann::json::object();
    data["steps"] = nlohmann::json{{"role", ok ? "ok" : "failed"}};
    data["member"] = nlohmann::json{{"uid", args["memberUid"]}, {"role", observed}};

    return command_result(command_id,
                          kOpGuildSetRole,
                          ok,
                          ok && observed.is_string() && observed.get<std::string>() == role,
                          true,
                          std::move(data));
}
}
