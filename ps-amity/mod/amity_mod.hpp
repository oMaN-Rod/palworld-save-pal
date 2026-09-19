#pragma once
#include <Mod/CppUserModBase.hpp>

#include "game_executor.hpp"

#include <amity/capability_registry.hpp>
#include <amity/command_queue.hpp>
#include <amity/config.hpp>
#include <amity/server.hpp>

#include <filesystem>
#include <memory>

#define AMITY_VERSION "0.3.1"
#define AMITY_STR_EXPAND(x) STR(x)

class AmityMod : public RC::CppUserModBase
{
public:
    AmityMod();
    ~AmityMod() override;
    auto on_unreal_init() -> void override;
    auto on_update() -> void override;

private:
    auto pump_game_thread() -> void;

    amity::CommandQueue m_queue;
    amity::CapabilityRegistry m_registry;
    amity::BridgeConfig m_config;
    GameExecutorImpl m_executor;
    std::unique_ptr<amity::BridgeServer> m_server;
    std::filesystem::path m_endpoint_dir;

    bool m_resolution_report_started{false};
    bool m_resolution_report_done{false};
    int m_ticks_since_last_resolution_attempt{0};
    int m_ticks_since_last_capability_refresh{0};

    bool m_pump_checked{false};
    int m_ticks_without_pump{0};
};
