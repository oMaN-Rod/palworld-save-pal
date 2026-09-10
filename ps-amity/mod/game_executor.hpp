#pragma once
#include <amity/command_queue.hpp>
#include <amity/game_port.hpp>

class GameExecutorImpl : public amity::GameExecutor
{
public:
    explicit GameExecutorImpl(const amity::CommandQueue& queue);

    amity::GameResponse execute(const amity::GameRequest& req) override;

private:
    const amity::CommandQueue& queue_;
};
