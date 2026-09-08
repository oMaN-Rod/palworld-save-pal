#pragma once

#include <functional>
#include <optional>
#include <string>
#include <string_view>

namespace amity {

struct BridgeConfig {
    std::string name = "PSPAmity";
    int port = 0;
    std::string token;
    std::string bind = "127.0.0.1";
};

using EnvLookup = std::function<std::optional<std::string>(const char*)>;

BridgeConfig parse_ini(std::string_view text);
void apply_env_overrides(BridgeConfig& cfg, const EnvLookup& lookup);
bool validate_config(const BridgeConfig& cfg, std::string& error);
bool is_loopback_bind(const std::string& bind);

}
