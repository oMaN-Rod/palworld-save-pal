#include <doctest/doctest.h>
#include <amity/command_queue.hpp>

#include <atomic>
#include <chrono>
#include <cstdint>
#include <future>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

namespace {

struct EchoExecutor : amity::GameExecutor {
    amity::GameResponse execute(const amity::GameRequest& req) override {
        amity::GameResponse r;
        r.data = nlohmann::json{{"echo", req.args}};
        return r;
    }
};

struct CountingExecutor : amity::GameExecutor {
    int calls = 0;
    amity::GameResponse execute(const amity::GameRequest&) override {
        ++calls;
        return amity::GameResponse{};
    }
};

struct FailFirstExecutor : amity::GameExecutor {
    int calls = 0;
    amity::GameResponse execute(const amity::GameRequest&) override {
        if (++calls == 1) {
            return amity::GameResponse::fail("not_authoritative", "this instance cannot write");
        }
        return amity::GameResponse{};
    }
};

struct ThrowFirstExecutor : amity::GameExecutor {
    int calls = 0;
    amity::GameResponse execute(const amity::GameRequest&) override {
        if (++calls == 1) {
            throw std::runtime_error("handler blew up");
        }
        return amity::GameResponse{};
    }
};

struct SlowExecutor : amity::GameExecutor {
    amity::GameResponse execute(const amity::GameRequest&) override {
        std::this_thread::sleep_for(std::chrono::milliseconds(3));
        return amity::GameResponse{};
    }
};

struct BlockFirstExecutor : amity::GameExecutor {
    std::promise<void> started;
    std::future<void> started_future = started.get_future();
    std::shared_future<void> release;
    bool signaled = false;

    explicit BlockFirstExecutor(std::shared_future<void> release_) : release(std::move(release_)) {}

    amity::GameResponse execute(const amity::GameRequest&) override {
        if (!signaled) {
            signaled = true;
            started.set_value();
            release.wait();
        }
        return amity::GameResponse{};
    }
};

}

TEST_CASE("submit result round-trips through drain") {
    amity::CommandQueue queue;
    EchoExecutor exec;

    auto future = queue.submit(amity::GameRequest{"do_thing", nlohmann::json{{"x", 42}}});
    CHECK(queue.depth() == 1);

    auto executed = queue.drain(exec, 10, std::chrono::milliseconds(1000));
    CHECK(executed == 1);
    CHECK(queue.depth() == 0);

    auto response = future.get();
    CHECK(response.ok());
    CHECK(response.data["echo"]["x"] == 42);
}

TEST_CASE("submit fails instantly with queue_full when saturated") {
    amity::CommandQueue queue(64);

    std::vector<std::future<amity::GameResponse>> futures;
    for (int i = 0; i < 64; ++i) {
        futures.push_back(queue.submit(amity::GameRequest{"op", nlohmann::json::object()}));
    }
    CHECK(queue.depth() == 64);

    auto overflow = queue.submit(amity::GameRequest{"op", nlohmann::json::object()});
    REQUIRE(overflow.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    auto response = overflow.get();
    REQUIRE_FALSE(response.ok());
    CHECK(response.error == "queue_full");
    CHECK(queue.depth() == 64);
}

TEST_CASE("drain executes at most max_items per call") {
    amity::CommandQueue queue;
    CountingExecutor exec;

    std::vector<std::future<amity::GameResponse>> futures;
    for (int i = 0; i < 6; ++i) {
        futures.push_back(queue.submit(amity::GameRequest{"op", nlohmann::json::object()}));
    }

    auto first = queue.drain(exec, 4, std::chrono::milliseconds(1000));
    CHECK(first == 4);
    CHECK(exec.calls == 4);
    CHECK(queue.depth() == 2);

    auto second = queue.drain(exec, 4, std::chrono::milliseconds(1000));
    CHECK(second == 2);
    CHECK(exec.calls == 6);
    CHECK(queue.depth() == 0);

    for (auto& f : futures) {
        CHECK(f.get().ok());
    }
}

TEST_CASE("drain stops between items once the budget is exceeded") {
    amity::CommandQueue queue;
    SlowExecutor exec;

    for (int i = 0; i < 4; ++i) {
        queue.submit(amity::GameRequest{"op", nlohmann::json::object()});
    }

    auto executed = queue.drain(exec, 4, std::chrono::milliseconds(2));
    CHECK(executed == 1);
    CHECK(queue.depth() == 3);
}

TEST_CASE("shutdown fails queued futures and rejects future submits") {
    amity::CommandQueue queue;
    auto pending = queue.submit(amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(queue.depth() == 1);

    queue.shutdown();

    CHECK(queue.depth() == 0);
    REQUIRE(pending.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    auto response = pending.get();
    REQUIRE_FALSE(response.ok());
    CHECK(response.error == "shutting_down");

    auto after = queue.submit(amity::GameRequest{"op", nlohmann::json::object()});
    REQUIRE(after.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    auto afterResponse = after.get();
    REQUIRE_FALSE(afterResponse.ok());
    CHECK(afterResponse.error == "shutting_down");

    queue.shutdown();
    CHECK(queue.depth() == 0);
}

TEST_CASE("depth tracks submit, drain, and shutdown") {
    amity::CommandQueue queue;
    CountingExecutor exec;

    CHECK(queue.depth() == 0);
    queue.submit(amity::GameRequest{"op", nlohmann::json::object()});
    queue.submit(amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(queue.depth() == 2);

    queue.drain(exec, 1, std::chrono::milliseconds(1000));
    CHECK(queue.depth() == 1);

    queue.shutdown();
    CHECK(queue.depth() == 0);
}

TEST_CASE("concurrent submits and a draining thread complete every future without loss") {
    constexpr int kProducers = 4;
    constexpr int kPerProducer = 50;
    constexpr int kTotal = kProducers * kPerProducer;

    amity::CommandQueue queue(kTotal);
    CountingExecutor exec;

    std::atomic<bool> stop{false};
    std::thread drainer([&]() {
        while (!stop.load()) {
            queue.drain(exec, 4, std::chrono::milliseconds(2));
        }
        while (queue.depth() > 0) {
            queue.drain(exec, 8, std::chrono::milliseconds(2));
        }
    });

    std::mutex futures_mutex;
    std::vector<std::future<amity::GameResponse>> futures;
    futures.reserve(kTotal);

    std::vector<std::thread> producers;
    for (int p = 0; p < kProducers; ++p) {
        producers.emplace_back([&]() {
            for (int i = 0; i < kPerProducer; ++i) {
                auto f = queue.submit(amity::GameRequest{"op", nlohmann::json::object()});
                std::lock_guard<std::mutex> lock(futures_mutex);
                futures.push_back(std::move(f));
            }
        });
    }
    for (auto& t : producers) {
        t.join();
    }

    stop.store(true);
    drainer.join();

    REQUIRE(futures.size() == static_cast<std::size_t>(kTotal));
    int okCount = 0;
    for (auto& f : futures) {
        auto response = f.get();
        if (response.ok()) {
            ++okCount;
        }
    }
    CHECK(okCount == kTotal);
    CHECK(queue.depth() == 0);
}

TEST_CASE("shutdown during drain fails in-flight batch leftovers with shutting_down") {
    amity::CommandQueue queue;
    std::promise<void> release_promise;
    BlockFirstExecutor exec(release_promise.get_future().share());

    auto f1 = queue.submit(amity::GameRequest{"op1", nlohmann::json::object()});
    auto f2 = queue.submit(amity::GameRequest{"op2", nlohmann::json::object()});
    auto f3 = queue.submit(amity::GameRequest{"op3", nlohmann::json::object()});
    REQUIRE(queue.depth() == 3);

    std::thread drainer([&]() { queue.drain(exec, 3, std::chrono::milliseconds(0)); });

    exec.started_future.wait();
    queue.shutdown();
    release_promise.set_value();
    drainer.join();

    REQUIRE(f1.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    CHECK(f1.get().ok());

    for (auto* f : {&f2, &f3}) {
        REQUIRE(f->wait_for(std::chrono::seconds(0)) == std::future_status::ready);
        auto response = f->get();
        REQUIRE_FALSE(response.ok());
        CHECK(response.error == "shutting_down");
    }

    CHECK(queue.depth() == 0);
}

TEST_CASE("saturation bound covers outstanding accepted work, not just queued items") {
    amity::CommandQueue queue(64);
    std::promise<void> release_promise;
    BlockFirstExecutor exec(release_promise.get_future().share());

    std::vector<std::future<amity::GameResponse>> futures;
    for (int i = 0; i < 64; ++i) {
        futures.push_back(queue.submit(amity::GameRequest{"op", nlohmann::json::object()}));
    }
    REQUIRE(queue.depth() == 64);

    std::thread drainer([&]() { queue.drain(exec, 4, std::chrono::milliseconds(1000)); });

    exec.started_future.wait();
    CHECK(queue.depth() == 60);

    for (int i = 0; i < 4; ++i) {
        auto overflow = queue.submit(amity::GameRequest{"op", nlohmann::json::object()});
        REQUIRE(overflow.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
        auto response = overflow.get();
        REQUIRE_FALSE(response.ok());
        CHECK(response.error == "queue_full");
    }

    release_promise.set_value();
    drainer.join();

    for (int i = 0; i < 4; ++i) {
        REQUIRE(futures[static_cast<std::size_t>(i)].wait_for(std::chrono::seconds(0)) == std::future_status::ready);
        CHECK(futures[static_cast<std::size_t>(i)].get().ok());
    }
    CHECK(queue.depth() == 60);

    auto after = queue.submit(amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(after.wait_for(std::chrono::seconds(0)) == std::future_status::timeout);
    CHECK(queue.depth() == 61);
}

TEST_CASE("submit_command joins in-flight duplicate submissions with the same hash") {
    amity::CommandQueue queue;
    std::promise<void> release_promise;
    BlockFirstExecutor exec(release_promise.get_future().share());

    auto first = queue.submit_command("cmd-1", 111, amity::GameRequest{"op", nlohmann::json{{"x", 1}}});
    auto second = queue.submit_command("cmd-1", 111, amity::GameRequest{"op", nlohmann::json{{"x", 1}}});
    CHECK(queue.depth() == 1);

    std::thread drainer([&]() { queue.drain(exec, 4, std::chrono::milliseconds(1000)); });
    exec.started_future.wait();
    release_promise.set_value();
    drainer.join();

    REQUIRE(first.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    REQUIRE(second.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    CHECK(first.get().ok());
    CHECK(second.get().ok());
}

TEST_CASE("submit_command serves a completed entry from cache without re-executing") {
    amity::CommandQueue queue;
    CountingExecutor exec;

    auto first = queue.submit_command("cmd-2", 222, amity::GameRequest{"op", nlohmann::json::object()});
    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    REQUIRE(first.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    CHECK(exec.calls == 1);

    auto second = queue.submit_command("cmd-2", 222, amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(queue.depth() == 0);
    REQUIRE(second.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    CHECK(exec.calls == 1);
}

TEST_CASE("submit_command does not cache a failed execution, so the same id retries and can succeed") {
    amity::CommandQueue queue;
    FailFirstExecutor exec;

    auto first = queue.submit_command("cmd-transient", 555, amity::GameRequest{"op", nlohmann::json::object()});
    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    REQUIRE(first.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    auto firstResponse = first.get();
    REQUIRE_FALSE(firstResponse.ok());
    CHECK(firstResponse.error == "not_authoritative");
    CHECK(exec.calls == 1);

    auto retry = queue.submit_command("cmd-transient", 555, amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(queue.depth() == 1);
    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    REQUIRE(retry.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    CHECK(retry.get().ok());
    CHECK(exec.calls == 2);

    auto third = queue.submit_command("cmd-transient", 555, amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(queue.depth() == 0);
    REQUIRE(third.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    CHECK(third.get().ok());
    CHECK(exec.calls == 2);
}

TEST_CASE("a throwing executor answers its own caller and leaves the queue usable") {
    amity::CommandQueue queue;
    ThrowFirstExecutor exec;

    auto threw = queue.submit_command("cmd-throw", 1, amity::GameRequest{"op", nlohmann::json::object()});
    CHECK_NOTHROW(queue.drain(exec, 4, std::chrono::milliseconds(1000)));
    REQUIRE(threw.wait_for(std::chrono::seconds(1)) == std::future_status::ready);

    amity::GameResponse response;
    CHECK_NOTHROW(response = threw.get());
    REQUIRE_FALSE(response.ok());
    CHECK(response.error == "game_error");
    CHECK(response.message.find("threw") != std::string::npos);

    std::shared_future<amity::GameResponse> next;
    CHECK_NOTHROW(next = queue.submit_command("cmd-after-throw", 2, amity::GameRequest{"op", nlohmann::json::object()}));
    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    REQUIRE(next.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    CHECK(next.get().ok());
    CHECK(exec.calls == 2);

    auto retried = queue.submit_command("cmd-throw", 1, amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(queue.depth() == 1);
    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    REQUIRE(retried.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    CHECK(retried.get().ok());
}

TEST_CASE("a failed execution frees its commandId for a different-args retry") {
    amity::CommandQueue queue;
    FailFirstExecutor exec;

    auto first = queue.submit_command("cmd-transient-args", 1, amity::GameRequest{"op", nlohmann::json::object()});
    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    REQUIRE_FALSE(first.get().ok());

    auto rearmed = queue.submit_command("cmd-transient-args", 2, amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(queue.depth() == 1);
    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    REQUIRE(rearmed.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    CHECK(rearmed.get().ok());
}

TEST_CASE("submit_command rejects a reused commandId with different args") {
    amity::CommandQueue queue;
    CountingExecutor exec;

    auto first = queue.submit_command("cmd-3", 333, amity::GameRequest{"op", nlohmann::json::object()});
    auto mismatched = queue.submit_command("cmd-3", 444, amity::GameRequest{"op", nlohmann::json::object()});

    REQUIRE(mismatched.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    auto response = mismatched.get();
    REQUIRE_FALSE(response.ok());
    CHECK(response.error == "validation_failed");
    CHECK(queue.depth() == 1);

    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    CHECK(exec.calls == 1);
    REQUIRE(first.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    CHECK(first.get().ok());
}

TEST_CASE("the 129th completed replay entry evicts the oldest and forces re-execution") {
    amity::CommandQueue queue;
    CountingExecutor exec;

    for (int i = 0; i < 129; ++i) {
        std::string id = "cmd-lru-" + std::to_string(i);
        auto f = queue.submit_command(id, static_cast<std::uint64_t>(i), amity::GameRequest{"op", nlohmann::json::object()});
        queue.drain(exec, 4, std::chrono::milliseconds(1000));
        REQUIRE(f.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    }
    CHECK(exec.calls == 129);

    auto resubmitEvicted = queue.submit_command("cmd-lru-0", 0, amity::GameRequest{"op", nlohmann::json::object()});
    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    REQUIRE(resubmitEvicted.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    CHECK(exec.calls == 130);

    auto resubmitCached = queue.submit_command("cmd-lru-128", 128, amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(queue.depth() == 0);
    REQUIRE(resubmitCached.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    CHECK(exec.calls == 130);
}

TEST_CASE("submit_command fails instantly with queue_full sharing the snapshot admission bound") {
    amity::CommandQueue queue(4);
    std::vector<std::future<amity::GameResponse>> snapshotFutures;
    for (int i = 0; i < 4; ++i) {
        snapshotFutures.push_back(queue.submit(amity::GameRequest{"op", nlohmann::json::object()}));
    }
    CHECK(queue.depth() == 4);

    auto overflow = queue.submit_command("cmd-overflow", 1, amity::GameRequest{"op", nlohmann::json::object()});
    REQUIRE(overflow.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    auto response = overflow.get();
    REQUIRE_FALSE(response.ok());
    CHECK(response.error == "queue_full");
}

TEST_CASE("submit_command does not cache an immediate queue_full failure, so a retry after draining executes fresh") {
    amity::CommandQueue queue(4);
    CountingExecutor exec;

    std::vector<std::future<amity::GameResponse>> fillers;
    for (int i = 0; i < 4; ++i) {
        fillers.push_back(queue.submit(amity::GameRequest{"op", nlohmann::json::object()}));
    }
    CHECK(queue.depth() == 4);

    auto first = queue.submit_command("cmd-retry", 1, amity::GameRequest{"op", nlohmann::json::object()});
    REQUIRE(first.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    auto firstResponse = first.get();
    REQUIRE_FALSE(firstResponse.ok());
    CHECK(firstResponse.error == "queue_full");
    CHECK(queue.depth() == 4);

    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    CHECK(queue.depth() == 0);
    CHECK(exec.calls == 4);
    for (auto& f : fillers) {
        CHECK(f.get().ok());
    }

    auto retry = queue.submit_command("cmd-retry", 1, amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(queue.depth() == 1);
    queue.drain(exec, 4, std::chrono::milliseconds(1000));
    REQUIRE(retry.wait_for(std::chrono::seconds(1)) == std::future_status::ready);
    CHECK(retry.get().ok());
    CHECK(exec.calls == 5);
}

TEST_CASE("submit_command fails a queued command on shutdown") {
    amity::CommandQueue queue;
    auto pending = queue.submit_command("cmd-shutdown", 1, amity::GameRequest{"op", nlohmann::json::object()});
    CHECK(queue.depth() == 1);

    queue.shutdown();

    REQUIRE(pending.wait_for(std::chrono::seconds(0)) == std::future_status::ready);
    auto response = pending.get();
    REQUIRE_FALSE(response.ok());
    CHECK(response.error == "shutting_down");
}

namespace {

struct DeferringExecutor : amity::GameExecutor {
    int waits = 0;
    int steps = 0;
    bool throw_in_step = false;

    amity::GameResponse execute(const amity::GameRequest&) override {
        return stage(waits);
    }

    amity::GameResponse stage(int remaining) {
        return amity::GameResponse::defer([this, remaining]() {
            ++steps;
            if (throw_in_step) {
                throw std::runtime_error("step blew up");
            }
            if (remaining > 0) {
                return stage(remaining - 1);
            }
            amity::GameResponse done;
            done.data = nlohmann::json{{"steps", steps}};
            return done;
        });
    }
};

bool settled(const std::future<amity::GameResponse>& future) {
    return future.wait_for(std::chrono::seconds(0)) == std::future_status::ready;
}

bool settled(const std::shared_future<amity::GameResponse>& future) {
    return future.wait_for(std::chrono::seconds(0)) == std::future_status::ready;
}

}

TEST_CASE("a pending response keeps its promise open until a later drain settles it") {
    amity::CommandQueue queue;
    DeferringExecutor exec;
    exec.waits = 0;
    auto future = queue.submit(amity::GameRequest{"op", {}, ""});

    queue.drain(exec, 4, std::chrono::milliseconds(50));
    CHECK_FALSE(settled(future));
    CHECK(exec.steps == 0);

    queue.drain(exec, 4, std::chrono::milliseconds(50));
    REQUIRE(settled(future));
    CHECK(future.get().data["steps"] == 1);
}

TEST_CASE("each stage hands the next one over and the drain count matches the waits") {
    amity::CommandQueue queue;
    DeferringExecutor exec;
    exec.waits = 3;
    auto future = queue.submit(amity::GameRequest{"op", {}, ""});

    queue.drain(exec, 4, std::chrono::milliseconds(50));
    for (int i = 0; i < 3; ++i) {
        queue.drain(exec, 4, std::chrono::milliseconds(50));
        CHECK_FALSE(settled(future));
    }
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    REQUIRE(settled(future));
    CHECK(future.get().data["steps"] == 4);
}

TEST_CASE("a deferred command that never finishes is failed as a timeout at the ceiling") {
    amity::CommandQueue queue(64, 3);
    DeferringExecutor exec;
    exec.waits = 100;
    auto future = queue.submit(amity::GameRequest{"op", {}, ""});

    queue.drain(exec, 4, std::chrono::milliseconds(50));
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    CHECK_FALSE(settled(future));
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    REQUIRE(settled(future));
    const amity::GameResponse response = future.get();
    CHECK_FALSE(response.ok());
    CHECK(response.error.value() == "timeout");
}

TEST_CASE("a throwing step settles its own promise and leaves the queue usable") {
    amity::CommandQueue queue;
    DeferringExecutor exec;
    exec.waits = 5;
    auto future = queue.submit(amity::GameRequest{"op", {}, ""});
    queue.drain(exec, 4, std::chrono::milliseconds(50));

    exec.throw_in_step = true;
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    REQUIRE(settled(future));
    CHECK(future.get().error.value() == "game_error");

    exec.throw_in_step = false;
    exec.waits = 0;
    auto next = queue.submit(amity::GameRequest{"op", {}, ""});
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    CHECK(settled(next));
    CHECK(next.get().ok());
}

TEST_CASE("a deferred command still counts against the admission bound until it settles") {
    amity::CommandQueue queue(1);
    DeferringExecutor exec;
    exec.waits = 1;
    auto first = queue.submit(amity::GameRequest{"op", {}, ""});
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    CHECK_FALSE(settled(first));

    auto refused = queue.submit(amity::GameRequest{"op", {}, ""});
    REQUIRE(settled(refused));
    CHECK(refused.get().error.value() == "queue_full");

    queue.drain(exec, 4, std::chrono::milliseconds(50));
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    REQUIRE(settled(first));

    auto admitted = queue.submit(amity::GameRequest{"op", {}, ""});
    CHECK_FALSE(settled(admitted));
}

TEST_CASE("shutdown fails a deferred command with shutting_down") {
    amity::CommandQueue queue;
    DeferringExecutor exec;
    exec.waits = 10;
    auto future = queue.submit(amity::GameRequest{"op", {}, ""});
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    CHECK_FALSE(settled(future));

    queue.shutdown();
    REQUIRE(settled(future));
    CHECK(future.get().error.value() == "shutting_down");
}

TEST_CASE("a replayed command id joins the deferred command and settles once with it") {
    amity::CommandQueue queue;
    DeferringExecutor exec;
    exec.waits = 1;
    auto first = queue.submit_command("cmd-1", 7, amity::GameRequest{"op", {}, "cmd-1"});
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    CHECK_FALSE(settled(first));

    auto again = queue.submit_command("cmd-1", 7, amity::GameRequest{"op", {}, "cmd-1"});
    CHECK_FALSE(settled(again));

    queue.drain(exec, 4, std::chrono::milliseconds(50));
    queue.drain(exec, 4, std::chrono::milliseconds(50));
    REQUIRE(settled(first));
    REQUIRE(settled(again));
    CHECK(again.get().data["steps"] == 2);
    CHECK(exec.steps == 2);
}
