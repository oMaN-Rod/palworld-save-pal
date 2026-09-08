#pragma once

#include <amity/capability_registry.hpp>
#include <amity/game_port.hpp>

#include <Unreal/UFunction.hpp>

#include <nlohmann/json.hpp>
#include <string>

namespace amity_rt
{
inline constexpr const char* kOpPalHeal = "pal.heal";
inline constexpr const char* kOpItemSetSlot = "item.setSlot";
inline constexpr const char* kOpPalRemove = "pal.remove";
inline constexpr const char* kOpPalMove = "pal.move";
inline constexpr const char* kOpPalAdd = "pal.add";
inline constexpr const char* kOpPalEdit = "pal.edit";
inline constexpr const char* kOpPlayerEdit = "player.edit";
inline constexpr const char* kOpGuildEdit = "guild.edit";
inline constexpr const char* kOpGuildSetRole = "guild.setRole";

bool is_write_op(const std::string& op);

amity::GameResponse command_pal_heal(const std::string& command_id, const nlohmann::json& args);
amity::GameResponse command_player_edit(const std::string& command_id, const nlohmann::json& args);
amity::GameResponse command_guild_edit(const std::string& command_id, const nlohmann::json& args);
amity::GameResponse command_guild_set_role(const std::string& command_id, const nlohmann::json& args);
amity::GameResponse command_item_set_slot(const std::string& command_id, const nlohmann::json& args);
amity::GameResponse command_pal_remove(const std::string& command_id, const nlohmann::json& args);
amity::GameResponse command_pal_move(const std::string& command_id, const nlohmann::json& args);
amity::GameResponse command_pal_add(const std::string& command_id, const nlohmann::json& args);
amity::GameResponse command_pal_edit(const std::string& command_id, const nlohmann::json& args);

bool add_item_layout_matches(RC::Unreal::UFunction* fn);

bool op_signature_ok(const std::string& op, std::string& reason);

bool read_op_signature_ok(const std::string& op, std::string& reason);

void seed_capabilities(amity::CapabilityRegistry& registry);
void refresh_capabilities(amity::CapabilityRegistry& registry);
}
