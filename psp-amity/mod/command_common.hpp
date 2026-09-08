#pragma once

#include "reflect.hpp"
#include "snapshots.hpp"

#include <amity/game_port.hpp>

#include <nlohmann/json.hpp>

#include <cstdint>
#include <optional>
#include <string>
#include <utility>

namespace amity_rt
{
inline constexpr const char* kStepOk = "ok";
inline constexpr const char* kStepFailed = "failed";
inline constexpr const char* kStepSkipped = "skipped";
inline constexpr const char* kStepUnresolved = "unresolved";

struct ObjectReturn
{
    RC::Unreal::UObject* ReturnValue{};
};

inline amity::GameResponse command_result(const std::string& command_id,
                                          const char* op,
                                          bool applied,
                                          bool verified,
                                          bool retry_safe,
                                          nlohmann::json data)
{
    amity::GameResponse response;
    response.data = nlohmann::json{
        {"commandId", command_id},
        {"op", op},
        {"applied", applied},
        {"verified", verified},
        {"retrySafe", retry_safe},
        {"data", std::move(data)},
    };
    return response;
}

inline bool parse_player_uid(const nlohmann::json& args, RC::Unreal::FGuid& player_uid, std::string& error)
{
    if (!args.contains("playerUid") || !args["playerUid"].is_string())
    {
        error = "playerUid is required";
        return false;
    }
    std::optional<RC::Unreal::FGuid> parsed = snap::parse_guid(args["playerUid"].get<std::string>());
    if (!parsed)
    {
        error = "playerUid is not a valid GUID";
        return false;
    }
    player_uid = *parsed;
    return true;
}

inline bool parse_guid_arg(const nlohmann::json& args, const char* key, RC::Unreal::FGuid& out, std::string& error)
{
    if (!args.contains(key) || !args[key].is_string())
    {
        error = std::string(key) + " is required";
        return false;
    }
    std::optional<RC::Unreal::FGuid> parsed = snap::parse_guid(args[key].get<std::string>());
    if (!parsed)
    {
        error = std::string(key) + " is not a valid GUID";
        return false;
    }
    out = *parsed;
    return true;
}

inline bool parse_bounded_int(const nlohmann::json& args,
                              const char* key,
                              int64_t min_value,
                              int64_t max_value,
                              int64_t& out,
                              std::string& error)
{
    if (!args.contains(key) || !args[key].is_number_integer())
    {
        error = std::string(key) + " is required and must be an integer";
        return false;
    }
    const int64_t value = args[key].get<int64_t>();
    if (value < min_value || value > max_value)
    {
        error = std::string(key) + " must be between " + std::to_string(min_value) + " and " + std::to_string(max_value);
        return false;
    }
    out = value;
    return true;
}

struct LivePlayer
{
    RC::Unreal::UObject* world_context{};
    RC::Unreal::UObject* player_state{};
};

inline std::optional<amity::GameResponse> require_live_player(const RC::Unreal::FGuid& player_uid, LivePlayer& out)
{
    out.world_context = any_player_controller();
    if (!out.world_context)
    {
        return amity::GameResponse::fail("capability_unavailable", "no live world context");
    }
    out.player_state = player_state_by_uid(out.world_context, player_uid);
    if (!out.player_state)
    {
        return amity::GameResponse::fail("validation_failed", "unknown playerUid");
    }
    return std::nullopt;
}

inline std::optional<amity::GameResponse> require_live_player(const nlohmann::json& args, LivePlayer& out)
{
    RC::Unreal::FGuid player_uid{};
    std::string error{};
    if (!parse_player_uid(args, player_uid, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }
    return require_live_player(player_uid, out);
}

inline RC::Unreal::UObject* guild_of(RC::Unreal::UObject* player_state)
{
    auto* prop = RC::Unreal::CastField<RC::Unreal::FObjectPropertyBase>(find_prop(player_state, {STR("GuildBelongTo")}));
    return prop ? prop->GetObjectPropertyValue(prop->ContainerPtrToValuePtr<void>(player_state)) : nullptr;
}

inline std::optional<RC::Unreal::FGuid> guild_id_of(RC::Unreal::UObject* guild)
{
    RC::Unreal::FGuid* id = guild ? snap::value_ptr<RC::Unreal::FGuid>(find_prop(guild, {STR("ID")}), guild) : nullptr;
    return id ? std::optional<RC::Unreal::FGuid>(*id) : std::nullopt;
}
}
