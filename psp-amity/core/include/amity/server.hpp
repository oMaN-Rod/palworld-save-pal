#pragma once

#include <amity/auth.hpp>
#include <amity/game_port.hpp>
#include <chrono>
#include <cstddef>
#include <functional>
#include <map>
#include <memory>
#include <mutex>
#include <nlohmann/json.hpp>
#include <string>

namespace ix {
class WebSocketServer;
class ConnectionState;
class WebSocket;
struct WebSocketMessage;
}

namespace amity {

struct ServerConfig {
    std::string bind = "127.0.0.1";
    int port = 0;
    std::string token;
    nlohmann::json hello_info;
    std::size_t max_frame = 262144;
    std::chrono::milliseconds request_wait{5000};
    std::function<nlohmann::json()> capabilities_provider;
    std::function<bool(const std::string& op, std::string& reason)> capability_check;
};

class BridgeServer {
public:
    BridgeServer(ServerConfig cfg, GamePort& port);
    ~BridgeServer();

    BridgeServer(const BridgeServer&) = delete;
    BridgeServer& operator=(const BridgeServer&) = delete;

    bool start(std::string& error);
    int port() const;
    void stop();

private:
    void handle_message(std::shared_ptr<ix::ConnectionState> state, ix::WebSocket& ws,
                         const std::unique_ptr<ix::WebSocketMessage>& msg);

    ServerConfig cfg_;
    GamePort& game_port_;
    std::unique_ptr<ix::WebSocketServer> server_;

    std::mutex sessions_mutex_;
    std::map<ix::ConnectionState*, std::unique_ptr<Session>> sessions_;
};

}
