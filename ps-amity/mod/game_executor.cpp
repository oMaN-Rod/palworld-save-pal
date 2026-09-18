#include "game_executor.hpp"

#include "amity_mod.hpp"
#include "game_commands.hpp"
#include "reflect.hpp"
#include "resolution_report.hpp"
#include "snapshots.hpp"

#include <amity/protocol.hpp>

#include <string>

using namespace RC::Unreal;

GameExecutorImpl::GameExecutorImpl(const amity::CommandQueue& queue) : queue_(queue) {}

amity::GameResponse GameExecutorImpl::execute(const amity::GameRequest& req)
{
    if (req.op == "status")
    {
        std::string mode;
        bool authoritative = false;
        amity_rt::resolve_mode(mode, authoritative);

        amity::GameResponse response;
        response.data = {
            {"mode", mode},
            {"authoritative", authoritative},
            {"worldLoaded", amity_rt::world_ready()},
            {"modVersion", AMITY_VERSION},
            {"protocolVersion", amity::PROTOCOL_VERSION},
            {"queueDepth", queue_.depth()},
        };
        return response;
    }

    if (req.op == "reflect")
    {
        return amity_rt::snapshot_reflect(req.args);
    }

    if (req.op == "build_info")
    {
        return amity_rt::snapshot_build_info();
    }

    if (req.op == "loaded_mods")
    {
        return amity_rt::snapshot_loaded_mods();
    }

    if (req.op == "resolution_report")
    {
        amity::GameResponse response;
        response.data = amity_rt::resolution_report_json();
        return response;
    }

    using Snapshot = amity::GameResponse (*)(const nlohmann::json&);
    struct ReadOp
    {
        const char* op;
        Snapshot read;
    };
    static const ReadOp kReads[] = {
        {"players", [](const nlohmann::json&) { return amity_rt::snapshot_players(); }},
        {"pals", amity_rt::snapshot_pals},
        {"pal_detail", amity_rt::snapshot_pal_detail},
        {"inventory", amity_rt::snapshot_inventory},
        {"guild", amity_rt::snapshot_guild},
        {"guilds", amity_rt::snapshot_guilds},
        {"base_pals", amity_rt::snapshot_base_pals},
        {"guild_containers", amity_rt::snapshot_guild_containers},
    };
    for (const ReadOp& read : kReads)
    {
        if (req.op != read.op)
        {
            continue;
        }
        if (!amity_rt::world_ready())
        {
            return amity::GameResponse::fail("capability_unavailable", "world not loaded");
        }
        return read.read(req.args);
    }

    if (amity_rt::is_write_op(req.op))
    {
        std::string mode;
        bool authoritative = false;
        amity_rt::resolve_mode(mode, authoritative);
        if (!authoritative)
        {
            return amity::GameResponse::fail("not_authoritative", "server is not authoritative for this world");
        }

        std::string reason;
        if (!amity_rt::op_signature_ok(req.op, reason))
        {
            return amity::GameResponse::fail("capability_unavailable", reason);
        }

        using Command = amity::GameResponse (*)(const std::string&, const nlohmann::json&);
        struct WriteOp
        {
            const char* op;
            Command run;
        };
        static const WriteOp kWrites[] = {
            {amity_rt::kOpPalHeal, amity_rt::command_pal_heal},
            {amity_rt::kOpItemSetSlot, amity_rt::command_item_set_slot},
            {amity_rt::kOpPalRemove, amity_rt::command_pal_remove},
            {amity_rt::kOpPalMove, amity_rt::command_pal_move},
            {amity_rt::kOpPalAdd, amity_rt::command_pal_add},
            {amity_rt::kOpPalEdit, amity_rt::command_pal_edit},
            {amity_rt::kOpPlayerEdit, amity_rt::command_player_edit},
            {amity_rt::kOpGuildEdit, amity_rt::command_guild_edit},
            {amity_rt::kOpGuildSetRole, amity_rt::command_guild_set_role},
        };
        for (const WriteOp& write : kWrites)
        {
            if (req.op == write.op)
            {
                return write.run(req.command_id, req.args);
            }
        }
        return amity::GameResponse::fail("capability_unavailable", "operation has no handler");
    }

    return amity::GameResponse::fail("capability_unavailable", "unknown operation");
}
