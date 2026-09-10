#pragma once

#include <cstdint>
#include <functional>
#include <future>
#include <memory>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <utility>

namespace amity {

struct GameRequest {
    std::string op;
    nlohmann::json args;
    std::string command_id;
};

struct GameContinuation;

struct GameResponse {
    nlohmann::json data;
    std::optional<std::string> error;
    std::string message;

    std::shared_ptr<GameContinuation> continuation;

    bool ok() const { return !error.has_value(); }
    bool pending() const { return continuation != nullptr; }

    static GameResponse fail(std::string code, std::string msg) {
        GameResponse r;
        r.error = std::move(code);
        r.message = std::move(msg);
        return r;
    }

    static GameResponse defer(std::function<GameResponse()> step);
};

struct GameContinuation {
    std::function<GameResponse()> step;
};

inline GameResponse GameResponse::defer(std::function<GameResponse()> step) {
    GameResponse r;
    r.continuation = std::make_shared<GameContinuation>(GameContinuation{std::move(step)});
    return r;
}

class GamePort {
public:
    virtual ~GamePort() = default;
    virtual std::future<GameResponse> submit(GameRequest req) = 0;

    virtual std::shared_future<GameResponse> submit_command(const std::string& command_id, std::uint64_t body_hash,
                                                              GameRequest req) {
        (void)command_id;
        (void)body_hash;
        return submit(std::move(req)).share();
    }
};

struct GameExecutor {
    virtual ~GameExecutor() = default;
    virtual GameResponse execute(const GameRequest& req) = 0;
};

}
