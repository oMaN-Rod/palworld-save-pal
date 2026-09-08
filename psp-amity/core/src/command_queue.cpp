#include <amity/command_queue.hpp>

#include <algorithm>
#include <exception>
#include <string>
#include <utility>

namespace amity {

CommandQueue::CommandQueue(std::size_t max_pending, std::size_t max_deferred_drains)
    : max_pending_(max_pending), max_deferred_drains_(max_deferred_drains) {}

void CommandQueue::step_deferred_locked_out() {
    std::deque<Deferred> working;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        working.swap(deferred_);
    }

    std::deque<Deferred> survivors;
    std::size_t settled = 0;
    for (auto& item : working) {
        GameResponse response;
        try {
            response = item.continuation->step();
        } catch (const std::exception& e) {
            response = GameResponse::fail("game_error", std::string("command step threw: ") + e.what());
        } catch (...) {
            response = GameResponse::fail("game_error", "command step threw an unknown exception");
        }

        if (response.pending()) {
            if (item.drains_left <= 1) {
                response = GameResponse::fail("timeout", "the command did not finish in time");
            } else {
                item.continuation = std::move(response.continuation);
                --item.drains_left;
                survivors.push_back(std::move(item));
                continue;
            }
        }
        item.promise.set_value(std::move(response));
        ++settled;
    }

    bool shutting_down = false;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        outstanding_ -= settled;
        shutting_down = shutting_down_;
        if (!shutting_down) {
            for (auto& item : survivors) {
                deferred_.push_back(std::move(item));
            }
        } else {
            outstanding_ -= survivors.size();
        }
    }
    if (shutting_down) {
        for (auto& item : survivors) {
            item.promise.set_value(GameResponse::fail("shutting_down", "command queue is shutting down"));
        }
    }
}

std::pair<std::future<GameResponse>, bool> CommandQueue::enqueue_locked(GameRequest req) {
    std::promise<GameResponse> promise;
    auto future = promise.get_future();
    if (shutting_down_) {
        promise.set_value(GameResponse::fail("shutting_down", "command queue is shutting down"));
        return {std::move(future), false};
    }
    if (outstanding_ >= max_pending_) {
        promise.set_value(GameResponse::fail("queue_full", "command queue is full"));
        return {std::move(future), false};
    }
    ++outstanding_;
    queue_.push_back(Pending{std::move(req), std::move(promise)});
    return {std::move(future), true};
}

std::future<GameResponse> CommandQueue::submit(GameRequest req) {
    std::lock_guard<std::mutex> lock(mutex_);
    return std::move(enqueue_locked(std::move(req)).first);
}

void CommandQueue::reap_completed_locked() {
    for (auto it = replay_.begin(); it != replay_.end();) {
        ReplayEntry& entry = it->second;
        if (!entry.future.valid()) {
            it = replay_.erase(it);
            continue;
        }
        if (!entry.completed && entry.future.wait_for(std::chrono::seconds(0)) == std::future_status::ready) {
            bool succeeded = false;
            try {
                succeeded = entry.future.get().ok();
            } catch (...) {
                succeeded = false;
            }
            if (!succeeded) {
                it = replay_.erase(it);
                continue;
            }
            entry.completed = true;
            completed_lru_.push_back(it->first);
            entry.lru_it = std::prev(completed_lru_.end());
        }
        ++it;
    }
    while (completed_lru_.size() > kMaxCompletedReplay) {
        replay_.erase(completed_lru_.front());
        completed_lru_.pop_front();
    }
}

std::shared_future<GameResponse> CommandQueue::submit_command(const std::string& command_id,
                                                                std::uint64_t body_hash, GameRequest req) {
    std::lock_guard<std::mutex> lock(mutex_);
    reap_completed_locked();

    auto it = replay_.find(command_id);
    if (it != replay_.end()) {
        if (it->second.hash == body_hash) {
            if (it->second.completed) {
                completed_lru_.splice(completed_lru_.end(), completed_lru_, it->second.lru_it);
            }
            return it->second.future;
        }
        std::promise<GameResponse> mismatch;
        mismatch.set_value(GameResponse::fail("validation_failed", "commandId reused with different args"));
        return mismatch.get_future().share();
    }

    auto [future, enqueued] = enqueue_locked(std::move(req));
    std::shared_future<GameResponse> shared = future.share();
    if (enqueued) {
        ReplayEntry entry;
        entry.hash = body_hash;
        entry.future = shared;
        replay_.emplace(command_id, std::move(entry));
    }
    return shared;
}

std::size_t CommandQueue::drain(GameExecutor& exec, std::size_t max_items, std::chrono::milliseconds budget) {
    step_deferred_locked_out();

    std::deque<Pending> batch;
    std::deque<Deferred> newly_deferred;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        std::size_t take = std::min(max_items, queue_.size());
        for (std::size_t i = 0; i < take; ++i) {
            batch.push_back(std::move(queue_.front()));
            queue_.pop_front();
        }
    }

    std::size_t executed = 0;
    const auto start = std::chrono::steady_clock::now();
    while (!batch.empty()) {
        if (executed > 0 && std::chrono::steady_clock::now() - start >= budget) {
            break;
        }
        Pending item = std::move(batch.front());
        batch.pop_front();
        GameResponse response;
        try {
            response = exec.execute(item.request);
        } catch (const std::exception& e) {
            response = GameResponse::fail("game_error", std::string("command handler threw: ") + e.what());
        } catch (...) {
            response = GameResponse::fail("game_error", "command handler threw an unknown exception");
        }
        if (response.pending()) {
            Deferred deferred;
            deferred.promise = std::move(item.promise);
            deferred.continuation = std::move(response.continuation);
            deferred.drains_left = max_deferred_drains_;
            newly_deferred.push_back(std::move(deferred));
            ++executed;
            continue;
        }
        {
            std::lock_guard<std::mutex> lock(mutex_);
            --outstanding_;
        }
        item.promise.set_value(std::move(response));
        ++executed;
    }

    if (!newly_deferred.empty()) {
        bool shutting_down = false;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            shutting_down = shutting_down_;
            if (!shutting_down) {
                for (auto& item : newly_deferred) {
                    deferred_.push_back(std::move(item));
                }
            } else {
                outstanding_ -= newly_deferred.size();
            }
        }
        if (shutting_down) {
            for (auto& item : newly_deferred) {
                item.promise.set_value(GameResponse::fail("shutting_down", "command queue is shutting down"));
            }
        }
    }

    if (!batch.empty()) {
        bool shutting_down = false;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            shutting_down = shutting_down_;
            if (shutting_down) {
                outstanding_ -= batch.size();
            } else {
                for (auto it = batch.rbegin(); it != batch.rend(); ++it) {
                    queue_.push_front(std::move(*it));
                }
            }
        }
        if (shutting_down) {
            for (auto& item : batch) {
                item.promise.set_value(GameResponse::fail("shutting_down", "command queue is shutting down"));
            }
        }
    }
    return executed;
}

void CommandQueue::shutdown() {
    std::deque<Pending> to_fail;
    std::deque<Deferred> deferred_to_fail;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        if (shutting_down_) {
            return;
        }
        shutting_down_ = true;
        to_fail.swap(queue_);
        deferred_to_fail.swap(deferred_);
        outstanding_ -= to_fail.size() + deferred_to_fail.size();
    }
    for (auto& item : to_fail) {
        item.promise.set_value(GameResponse::fail("shutting_down", "command queue is shutting down"));
    }
    for (auto& item : deferred_to_fail) {
        item.promise.set_value(GameResponse::fail("shutting_down", "command queue is shutting down"));
    }
}

std::size_t CommandQueue::depth() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return queue_.size();
}

}
