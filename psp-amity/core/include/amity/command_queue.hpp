#pragma once

#include <amity/game_port.hpp>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <deque>
#include <future>
#include <list>
#include <mutex>
#include <string>
#include <unordered_map>
#include <utility>

namespace amity {

class CommandQueue : public GamePort {
public:
    explicit CommandQueue(std::size_t max_pending = 64, std::size_t max_deferred_drains = 600);

    std::future<GameResponse> submit(GameRequest req) override;
    std::shared_future<GameResponse> submit_command(const std::string& command_id, std::uint64_t body_hash,
                                                      GameRequest req) override;
    std::size_t drain(GameExecutor& exec, std::size_t max_items, std::chrono::milliseconds budget);
    void shutdown();

    std::size_t depth() const;

private:
    struct Pending {
        GameRequest request;
        std::promise<GameResponse> promise;
    };

    struct Deferred {
        std::promise<GameResponse> promise;
        std::shared_ptr<GameContinuation> continuation;
        std::size_t drains_left = 0;
    };

    struct ReplayEntry {
        std::uint64_t hash = 0;
        std::shared_future<GameResponse> future;
        bool completed = false;
        std::list<std::string>::iterator lru_it;
    };

    static constexpr std::size_t kMaxCompletedReplay = 128;

    std::pair<std::future<GameResponse>, bool> enqueue_locked(GameRequest req);
    void reap_completed_locked();
    void step_deferred_locked_out();

    mutable std::mutex mutex_;
    std::deque<Pending> queue_;
    std::size_t max_pending_;
    std::size_t max_deferred_drains_;
    std::deque<Deferred> deferred_;
    std::size_t outstanding_ = 0;
    bool shutting_down_ = false;

    std::unordered_map<std::string, ReplayEntry> replay_;
    std::list<std::string> completed_lru_;
};

}
