#pragma once

#include <amity/config.hpp>

#include <filesystem>

namespace amity {

std::filesystem::path config_path_beside_module(const void* address_in_module);
BridgeConfig load_config(const std::filesystem::path& ini_path);

/// The running process's own executable, not this DLL's — a workshop install
/// on Game Pass keeps Amity's config beside a `WinGDK`-free path, but the game
/// executable itself always sits under `Pal/Binaries/WinGDK`.
std::filesystem::path game_executable_path();

}
