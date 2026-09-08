#pragma once

#include <amity/config.hpp>

#include <filesystem>

namespace amity {

std::filesystem::path config_path_beside_module(const void* address_in_module);
BridgeConfig load_config(const std::filesystem::path& ini_path);

}
