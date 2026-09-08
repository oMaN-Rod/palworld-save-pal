#pragma once
#include <functional>

namespace amity_rt
{
// UE4SS drives on_update from its own event loop thread, so anything that touches a UObject has to
// be handed to the game thread instead of running there.
void install_game_thread_pump(std::function<void()> pump);
void stop_game_thread_pump();
bool game_thread_pumped();
bool world_transitioning();
}
