#include "snapshots.hpp"

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt::snap;

namespace
{
constexpr int32_t kMaxPlayers = 1024;

struct GetAllPlayerStatesParams
{
    UObject* WorldContextObject{};
    TArray<UObject*> OutPlayerStates{};
};

struct NoParamsObjectReturn
{
    UObject* ReturnValue{};
};

struct NoParamsVectorReturn
{
    FVector ReturnValue{};
};

struct NoParamsRotatorReturn
{
    FRotator ReturnValue{};
};

nlohmann::json read_player_level(const SaveParameterView& save_parameter)
{
    uint8_t* level = value_ptr<uint8_t>(amity_rt::find_struct_prop(save_parameter.type, {STR("Level")}), save_parameter.ptr);
    if (!level)
    {
        return nullptr;
    }
    return static_cast<int>(*level);
}
}

namespace amity_rt
{
amity::GameResponse snapshot_players()
{
    amity::GameResponse response;

    UObject* world_context = any_player_controller();
    UFunction* get_all_fn = find_function(STR("/Script/Pal.PalUtility:GetAllPlayerStates"));
    if (!world_context || !get_all_fn)
    {
        response.data = {{"players", nlohmann::json::array()}, {"status", "unavailable"}};
        return response;
    }

    GetAllPlayerStatesParams list_params{};
    list_params.WorldContextObject = world_context;
    world_context->ProcessEvent(get_all_fn, &list_params);

    UFunction* get_controller_fn = find_function(STR("/Script/Engine.PlayerState:GetPlayerController"));
    UFunction* get_pawn_fn = find_function(STR("/Script/Engine.Controller:K2_GetPawn"));
    UFunction* get_location_fn = find_function(STR("/Script/Engine.Actor:K2_GetActorLocation"));
    UFunction* get_rotation_fn = find_function(STR("/Script/Engine.Actor:K2_GetActorRotation"));
    UFunction* get_char_manager_fn = find_function(STR("/Script/Pal.PalUtility:GetCharacterManager"));
    UFunction* get_individual_param_fn = find_function(STR("/Script/Pal.PalCharacterManager:GetIndividualCharacterParameter"));

    nlohmann::json players = nlohmann::json::array();
    bool list_partial = false;

    const int32_t count = list_params.OutPlayerStates.Num();
    const int32_t limit = count < kMaxPlayers ? count : kMaxPlayers;
    for (int32_t i = 0; i < limit; ++i)
    {
        UObject* player_state = list_params.OutPlayerStates[i];
        if (!player_state)
        {
            continue;
        }

        bool player_ok = true;
        nlohmann::json entry = nlohmann::json::object();

        if (FGuid* uid = value_ptr<FGuid>(find_prop(player_state, {STR("PlayerUId")}), player_state))
        {
            entry["uid"] = format_guid(*uid);
        }
        else
        {
            entry["uid"] = nullptr;
            player_ok = false;
        }

        if (FString* nickname = value_ptr<FString>(find_prop(player_state, {STR("PlayerNamePrivate")}), player_state))
        {
            entry["nickname"] = to_utf8(nickname->operator*());
        }
        else
        {
            entry["nickname"] = nullptr;
            player_ok = false;
        }

        FProperty* guild_prop = find_prop(player_state, {STR("GuildBelongTo")});
        if (auto* guild_obj_prop = CastField<FObjectPropertyBase>(guild_prop))
        {
            UObject* guild = guild_obj_prop->GetObjectPropertyValue(guild_obj_prop->ContainerPtrToValuePtr<void>(player_state));
            if (guild)
            {
                if (FGuid* guild_id = value_ptr<FGuid>(find_prop(guild, {STR("ID")}), guild))
                {
                    entry["guildId"] = format_guid(*guild_id);
                }
                else
                {
                    entry["guildId"] = nullptr;
                    player_ok = false;
                }
            }
            else
            {
                entry["guildId"] = nullptr;
            }
        }
        else
        {
            entry["guildId"] = nullptr;
            player_ok = false;
        }

        const SaveParameterView save_parameter =
            player_save_parameter(world_context, player_state, get_char_manager_fn, get_individual_param_fn);

        nlohmann::json level = read_player_level(save_parameter);
        entry["level"] = level;
        if (level.is_null())
        {
            player_ok = false;
        }

        const std::optional<int64_t> exp = read_player_exp(save_parameter);
        entry["exp"] = exp ? nlohmann::json(*exp) : nlohmann::json(nullptr);
        if (!exp)
        {
            player_ok = false;
        }

        bool has_position = false;
        double x = 0.0, y = 0.0, z = 0.0;
        bool has_yaw = false;
        double yaw = 0.0;
        if (get_controller_fn && get_location_fn)
        {
            NoParamsObjectReturn controller_params{};
            player_state->ProcessEvent(get_controller_fn, &controller_params);
            UObject* controller = controller_params.ReturnValue;

            UObject* actor_for_location = controller;
            if (controller && get_pawn_fn)
            {
                NoParamsObjectReturn pawn_params{};
                controller->ProcessEvent(get_pawn_fn, &pawn_params);
                if (pawn_params.ReturnValue)
                {
                    actor_for_location = pawn_params.ReturnValue;
                }
            }

            if (actor_for_location)
            {
                NoParamsVectorReturn location_params{};
                actor_for_location->ProcessEvent(get_location_fn, &location_params);
                x = location_params.ReturnValue.X();
                y = location_params.ReturnValue.Y();
                z = location_params.ReturnValue.Z();
                has_position = true;

                if (get_rotation_fn)
                {
                    NoParamsRotatorReturn rotation_params{};
                    actor_for_location->ProcessEvent(get_rotation_fn, &rotation_params);
                    yaw = rotation_params.ReturnValue.GetYaw();
                    has_yaw = true;
                }
            }
        }

        entry["yaw"] = has_yaw ? nlohmann::json(yaw) : nullptr;

        if (has_position)
        {
            entry["x"] = x;
            entry["y"] = y;
            entry["z"] = z;
        }
        else
        {
            entry["x"] = nullptr;
            entry["y"] = nullptr;
            entry["z"] = nullptr;
            player_ok = false;
        }

        entry["status"] = player_ok ? "ok" : "partial";
        if (!player_ok)
        {
            list_partial = true;
        }
        players.push_back(std::move(entry));
    }

    response.data = {{"players", players}, {"status", list_partial ? "partial" : "ok"}};
    return response;
}
}
