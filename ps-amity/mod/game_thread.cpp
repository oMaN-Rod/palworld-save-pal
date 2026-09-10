#include "game_thread.hpp"

#include <Unreal/FURL.hpp>
#include <Unreal/FWorldContext.hpp>
#include <Unreal/Hooks/Hooks.hpp>

#include <atomic>
#include <mutex>
#include <utility>

using namespace RC;
using namespace RC::Unreal;

namespace
{
std::mutex g_pump_mutex{};
std::function<void()> g_pump{};
bool g_running{false};
std::atomic<bool> g_pumped{false};
std::atomic<int> g_transitions{0};

Hook::FCallbackOptions callback_options(const wchar_t* hook_name)
{
    Hook::FCallbackOptions options{};
    options.bReadonly = true;
    options.OwnerModName = STR("PSAmity");
    options.HookName = hook_name;
    return options;
}
}

namespace amity_rt
{
void install_game_thread_pump(std::function<void()> pump)
{
    {
        std::lock_guard<std::mutex> lock(g_pump_mutex);
        g_pump = std::move(pump);
        g_running = true;
    }

    Hook::RegisterEngineTickPostCallback(
        [](Hook::TCallbackIterationData<void>&, UEngine*, float, bool) {
            std::lock_guard<std::mutex> lock(g_pump_mutex);
            if (!g_running)
            {
                return;
            }
            g_pumped.store(true, std::memory_order_release);
            g_pump();
        },
        callback_options(STR("GameThreadPump")));

    Hook::RegisterLoadMapPreCallback(
        [](Hook::TCallbackIterationData<bool>&, UEngine*, FWorldContext&, FURL, UPendingNetGame*, FString&) {
            g_transitions.fetch_add(1, std::memory_order_acq_rel);
        },
        callback_options(STR("WorldTeardown")));

    Hook::RegisterLoadMapPostCallback(
        [](Hook::TCallbackIterationData<bool>&, UEngine*, FWorldContext&, FURL, UPendingNetGame*, FString&) {
            if (g_transitions.fetch_sub(1, std::memory_order_acq_rel) <= 0)
            {
                g_transitions.store(0, std::memory_order_release);
            }
        },
        callback_options(STR("WorldReady")));
}

void stop_game_thread_pump()
{
    std::lock_guard<std::mutex> lock(g_pump_mutex);
    g_running = false;
    g_pump = nullptr;
}

bool game_thread_pumped()
{
    return g_pumped.load(std::memory_order_acquire);
}

bool world_transitioning()
{
    return g_transitions.load(std::memory_order_acquire) > 0;
}
}
