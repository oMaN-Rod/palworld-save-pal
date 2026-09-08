#include <amity/server.hpp>

#include <amity/protocol.hpp>
#include <amity/token.hpp>
#include <ixwebsocket/IXNetSystem.h>
#include <ixwebsocket/IXWebSocketServer.h>

#include <cstdint>
#include <exception>
#include <future>
#include <utility>

namespace amity {

namespace {

bool normalize_bind(const std::string& bind, std::string& normalized, std::string& error) {
    if (bind == "localhost") {
        normalized = "127.0.0.1";
        return true;
    }
    sockaddr_in probe{};
    if (::inet_pton(AF_INET, bind.c_str(), &probe.sin_addr) != 1) {
        error = "bind must be 127.0.0.1, localhost, 0.0.0.0 or an IPv4 address";
        return false;
    }
    normalized = bind;
    return true;
}

std::string reply_type_for(const std::string& request_type) {
    const std::string prefix = "get_";
    if (request_type.size() > prefix.size() && request_type.rfind(prefix, 0) == 0) {
        return request_type.substr(prefix.size());
    }
    return request_type;
}

// IXWebSocket's WebSocketServer does not support ephemeral-port discovery: passing port 0
// to its constructor binds the OS-chosen port, but getPort() just echoes the configured
// value back (still 0), never the real one. So the real port is discovered up front by
// binding a throwaway probe socket to port 0, reading it back with getsockname, and closing
// it before the real server binds the same port number. This is a TOCTOU race in principle
// (another process could grab or start serving the port in the gap) but is accepted for this
// version, as is the equivalent race in the already-serving check below.
bool find_ephemeral_port(const std::string& host, int& out_port, std::string& error) {
    SOCKET probe = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (probe == INVALID_SOCKET) {
        error = "failed to create ephemeral-port probe socket";
        return false;
    }

    sockaddr_in addr{};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(0);
    if (::inet_pton(AF_INET, host.c_str(), &addr.sin_addr) != 1) {
        closesocket(probe);
        error = "invalid bind address for ephemeral-port probe";
        return false;
    }

    if (bind(probe, reinterpret_cast<sockaddr*>(&addr), sizeof(addr)) != 0) {
        closesocket(probe);
        error = "failed to bind ephemeral-port probe socket";
        return false;
    }

    int len = sizeof(addr);
    if (getsockname(probe, reinterpret_cast<sockaddr*>(&addr), &len) != 0) {
        closesocket(probe);
        error = "failed to read ephemeral port from probe socket";
        return false;
    }

    out_port = ntohs(addr.sin_port);
    closesocket(probe);
    return true;
}

// A bind-based check would false-positive on a TIME_WAIT entry from a prior connection, which
// accepts no new connections but still occupies the port for a bind. Connecting instead avoids
// that: only something actually listening will accept.
bool port_already_serving(const std::string& host, int port) {
    std::string probe_host = host == "0.0.0.0" ? "127.0.0.1" : host;

    SOCKET probe = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (probe == INVALID_SOCKET) {
        return false;
    }

    u_long non_blocking = 1;
    ioctlsocket(probe, FIONBIO, &non_blocking);

    sockaddr_in addr{};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(static_cast<u_short>(port));
    if (::inet_pton(AF_INET, probe_host.c_str(), &addr.sin_addr) != 1) {
        closesocket(probe);
        return false;
    }

    if (connect(probe, reinterpret_cast<sockaddr*>(&addr), sizeof(addr)) == 0) {
        closesocket(probe);
        return true;
    }

    if (WSAGetLastError() != WSAEWOULDBLOCK) {
        closesocket(probe);
        return false;
    }

    fd_set writable{};
    FD_ZERO(&writable);
    FD_SET(probe, &writable);
    timeval timeout{0, 250000};
    if (select(0, nullptr, &writable, nullptr, &timeout) <= 0) {
        closesocket(probe);
        return false;
    }

    int so_error = 0;
    int so_error_len = sizeof(so_error);
    getsockopt(probe, SOL_SOCKET, SO_ERROR, reinterpret_cast<char*>(&so_error), &so_error_len);
    closesocket(probe);
    return so_error == 0;
}

}

BridgeServer::BridgeServer(ServerConfig cfg, GamePort& port) : cfg_(std::move(cfg)), game_port_(port) {}

BridgeServer::~BridgeServer() {
    stop();
}

bool BridgeServer::start(std::string& error) {
    if (cfg_.token.empty()) {
        error = "token must not be empty";
        return false;
    }
    std::string bind_host;
    if (!normalize_bind(cfg_.bind, bind_host, error)) {
        return false;
    }

    int listen_port = cfg_.port;
    if (listen_port == 0) {
        if (!find_ephemeral_port(bind_host, listen_port, error)) {
            return false;
        }
    } else if (port_already_serving(bind_host, listen_port)) {
        error = "port " + std::to_string(listen_port) + " on " + bind_host + " is already serving another connection";
        return false;
    }

    auto server = std::make_unique<ix::WebSocketServer>(listen_port, bind_host);
    server->setOnClientMessageCallback(
        [this](std::shared_ptr<ix::ConnectionState> state, ix::WebSocket& ws,
               const ix::WebSocketMessagePtr& msg) { handle_message(std::move(state), ws, msg); });

    auto result = server->listen();
    if (!result.first) {
        error = "failed to listen on " + bind_host + ":" + std::to_string(listen_port) + ": " + result.second;
        return false;
    }
    server->start();
    server_ = std::move(server);
    return true;
}

int BridgeServer::port() const {
    return server_ ? server_->getPort() : 0;
}

void BridgeServer::stop() {
    if (!server_) {
        return;
    }
    server_->stop();
    server_.reset();
}

void BridgeServer::handle_message(std::shared_ptr<ix::ConnectionState> state, ix::WebSocket& ws,
                                   const std::unique_ptr<ix::WebSocketMessage>& msg) {
    ix::ConnectionState* key = state.get();

    try {
        handle_message_inner(key, ws, msg);
    } catch (const std::exception&) {
        ws.close();
        std::lock_guard<std::mutex> lock(sessions_mutex_);
        sessions_.erase(key);
    } catch (...) {
        ws.close();
        std::lock_guard<std::mutex> lock(sessions_mutex_);
        sessions_.erase(key);
    }
}

void BridgeServer::handle_message_inner(ix::ConnectionState* key, ix::WebSocket& ws,
                                         const std::unique_ptr<ix::WebSocketMessage>& msg) {
    if (msg->type == ix::WebSocketMessageType::Open) {
        std::lock_guard<std::mutex> lock(sessions_mutex_);
        sessions_[key] = std::make_unique<Session>(cfg_.token, generate_token_hex(), cfg_.hello_info);
        return;
    }

    if (msg->type == ix::WebSocketMessageType::Close || msg->type == ix::WebSocketMessageType::Error) {
        std::lock_guard<std::mutex> lock(sessions_mutex_);
        sessions_.erase(key);
        return;
    }

    if (msg->type != ix::WebSocketMessageType::Message) {
        return;
    }

    if (msg->str.size() > cfg_.max_frame) {
        ws.close();
        std::lock_guard<std::mutex> lock(sessions_mutex_);
        sessions_.erase(key);
        return;
    }

    Session* session = nullptr;
    {
        std::lock_guard<std::mutex> lock(sessions_mutex_);
        auto it = sessions_.find(key);
        if (it == sessions_.end()) {
            return;
        }
        session = it->second.get();
    }

    bool was_ready = session->ready();
    Session::Output output = session->on_message(msg->str);
    bool became_ready = !was_ready && session->ready();

    if (output.close) {
        for (const auto& frame : output.send) {
            ws.send(frame);
        }
        ws.close();
        std::lock_guard<std::mutex> lock(sessions_mutex_);
        sessions_.erase(key);
        return;
    }

    for (const auto& frame : output.send) {
        ws.send(frame);
    }

    if (became_ready) {
        nlohmann::json snapshot = cfg_.capabilities_provider
            ? cfg_.capabilities_provider()
            : nlohmann::json{{"version", 0}, {"ops", nlohmann::json::object()}};
        ws.send(serialize_envelope(Envelope{"push", "capabilities", std::move(snapshot)}));
    }

    if (!output.request) {
        return;
    }

    const Envelope& req = *output.request;

    if (req.type == "get_capabilities") {
        nlohmann::json snapshot = cfg_.capabilities_provider
            ? cfg_.capabilities_provider()
            : nlohmann::json{{"version", 0}, {"ops", nlohmann::json::object()}};
        ws.send(make_reply(req.id, "capabilities", std::move(snapshot)));
        return;
    }

    if (req.type == "command") {
        if (!req.data.contains("commandId") || !req.data["commandId"].is_string()) {
            ws.send(make_error(req.id, "validation_failed", "commandId must be a non-empty string"));
            return;
        }
        std::string command_id = req.data["commandId"].get<std::string>();
        if (command_id.empty() || command_id.size() > 128) {
            ws.send(make_error(req.id, "validation_failed", "commandId must be 1-128 characters"));
            return;
        }

        if (!req.data.contains("op") || !req.data["op"].is_string() || req.data["op"].get<std::string>().empty()) {
            ws.send(make_error(req.id, "validation_failed", "op must be a non-empty string"));
            return;
        }
        std::string op = req.data["op"].get<std::string>();

        nlohmann::json args = nlohmann::json::object();
        if (req.data.contains("args")) {
            if (!req.data["args"].is_object()) {
                ws.send(make_error(req.id, "validation_failed", "args must be an object"));
                return;
            }
            args = req.data["args"];
        }

        if (cfg_.capability_check) {
            std::string reason;
            if (!cfg_.capability_check(op, reason)) {
                ws.send(make_error(req.id, "capability_unavailable", reason));
                return;
            }
        }

        std::uint64_t body_hash = std::hash<std::string>{}(args.dump());

        std::shared_future<GameResponse> future =
            game_port_.submit_command(command_id, body_hash, GameRequest{op, args, command_id});

        if (future.wait_for(cfg_.request_wait) == std::future_status::timeout) {
            ws.send(make_error(req.id, "timeout", "request timed out; the command may still be running"));
            return;
        }

        GameResponse response = future.get();
        if (!response.ok()) {
            ws.send(make_error(req.id, *response.error, response.message));
        } else {
            ws.send(make_reply(req.id, "command_result", response.data));
        }
        return;
    }

    std::string reply_type = reply_type_for(req.type);
    auto future = game_port_.submit(GameRequest{reply_type, req.data});
    if (future.wait_for(cfg_.request_wait) == std::future_status::timeout) {
        ws.send(make_error(req.id, "timeout", "request timed out; the command may still be running"));
        return;
    }

    GameResponse response = future.get();
    if (!response.ok()) {
        ws.send(make_error(req.id, *response.error, response.message));
    } else {
        ws.send(make_reply(req.id, reply_type, response.data));
    }
}

}
