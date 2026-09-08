#include "amity_mod.hpp"
#include "game_commands.hpp"
#include "game_thread.hpp"
#include "reflect.hpp"
#include "resolution_report.hpp"

#include <amity/endpoint_file.hpp>
#include <amity/token.hpp>

#include <DynamicOutput/DynamicOutput.hpp>

#include <chrono>
#include <stdexcept>
#include <string>
#include <utility>

using namespace RC;

AmityMod::AmityMod() : m_queue(), m_registry(), m_executor(m_queue)
{
    ModName = STR("PSPAmity");
    ModVersion = AMITY_STR_EXPAND(AMITY_VERSION);
    ModDescription = STR("PSP live game bridge");
    ModAuthors = STR("PSP");
    Output::send<LogLevel::Verbose>(STR("[PSPAmity] loaded v{}\n"), AMITY_STR_EXPAND(AMITY_VERSION));
}

AmityMod::~AmityMod()
{
    amity_rt::stop_game_thread_pump();
    m_queue.shutdown();
    if (m_server)
    {
        m_server->stop();
    }
    if (!m_endpoint_dir.empty())
    {
        amity::remove_endpoint_file(m_endpoint_dir);
    }
}

auto AmityMod::on_unreal_init() -> void
{
    Output::send<LogLevel::Verbose>(STR("[PSPAmity] unreal initialized\n"));

    std::string token;
    try
    {
        token = amity::generate_token_hex();
    }
    catch (const std::exception& e)
    {
        Output::send<LogLevel::Error>(STR("[PSPAmity] failed to generate bridge token: {}\n"), amity_rt::widen(e.what()));
        return;
    }

    m_endpoint_dir = amity::default_endpoint_dir();
    if (m_endpoint_dir.empty())
    {
        Output::send<LogLevel::Error>(
            STR("[PSPAmity] endpoint directory unresolved (LOCALAPPDATA missing); bridge server not started\n"));
        return;
    }

    amity_rt::seed_capabilities(m_registry);

    amity::ServerConfig cfg;
    cfg.token = token;
    cfg.hello_info = {{"mod", "PSPAmity"}, {"version", AMITY_VERSION}};
    amity::CapabilityRegistry* registry = &m_registry;
    cfg.capabilities_provider = [registry] { return registry->snapshot(); };
    cfg.capability_check = [registry](const std::string& op, std::string& reason) { return registry->available(op, reason); };

    m_server = std::make_unique<amity::BridgeServer>(std::move(cfg), m_queue);

    std::string error;
    if (!m_server->start(error))
    {
        Output::send<LogLevel::Error>(STR("[PSPAmity] failed to start bridge server: {}\n"), amity_rt::widen(error));
        m_server.reset();
        return;
    }

    if (!amity::write_endpoint_file(m_endpoint_dir, m_server->port(), token, "PSPAmity", "127.0.0.1", error))
    {
        Output::send<LogLevel::Error>(STR("[PSPAmity] failed to write endpoint file: {}\n"), amity_rt::widen(error));
        m_server->stop();
        m_server.reset();
        return;
    }

    Output::send<LogLevel::Verbose>(STR("[PSPAmity] bridge listening on 127.0.0.1:{}\n"), m_server->port());

    amity_rt::install_game_thread_pump([this] { pump_game_thread(); });
}

namespace
{
constexpr int kResolutionRetryTicks = 60;
constexpr int kCapabilityRefreshTicks = 60;
constexpr int kPumpWarningTicks = 1000;
}

auto AmityMod::on_update() -> void
{
    if (m_pump_checked || !m_server)
    {
        return;
    }
    if (amity_rt::game_thread_pumped())
    {
        m_pump_checked = true;
        return;
    }
    if (++m_ticks_without_pump < kPumpWarningTicks)
    {
        return;
    }
    m_pump_checked = true;
    Output::send<LogLevel::Error>(STR("[PSPAmity] engine tick hook never fired; live game features are unavailable\n"));
}

auto AmityMod::pump_game_thread() -> void
{
    m_queue.drain(m_executor, 4, std::chrono::milliseconds(2));

    if (++m_ticks_since_last_capability_refresh >= kCapabilityRefreshTicks)
    {
        m_ticks_since_last_capability_refresh = 0;
        amity_rt::refresh_capabilities(m_registry);
    }

    if (m_resolution_report_done)
    {
        return;
    }

    if (!m_resolution_report_started)
    {
        if (!amity_rt::world_ready())
        {
            return;
        }
        m_resolution_report_started = true;
        m_ticks_since_last_resolution_attempt = 0;
    }
    else
    {
        if (++m_ticks_since_last_resolution_attempt < kResolutionRetryTicks)
        {
            return;
        }
        m_ticks_since_last_resolution_attempt = 0;
    }

    m_resolution_report_done = amity_rt::update_resolution_report();
}
