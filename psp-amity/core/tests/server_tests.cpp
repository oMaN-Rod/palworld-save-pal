#include <doctest/doctest.h>
#include <amity/hmac.hpp>
#include <amity/server.hpp>
#include <amity/protocol.hpp>

#include <ixwebsocket/IXWebSocket.h>

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <deque>
#include <future>
#include <mutex>
#include <optional>
#include <string>
#include <vector>

namespace {

struct StubGamePort : amity::GamePort {
    nlohmann::json canned_data = nlohmann::json{{"players", 4}};
    std::optional<std::string> failure_code;
    std::string failure_message;
    amity::GameRequest last_request;

    std::future<amity::GameResponse> submit(amity::GameRequest req) override {
        last_request = req;
        std::promise<amity::GameResponse> promise;
        amity::GameResponse response;
        if (failure_code) {
            response.error = failure_code;
            response.message = failure_message;
        } else {
            response.data = canned_data;
        }
        promise.set_value(response);
        return promise.get_future();
    }
};

struct NeverFulfillingGamePort : amity::GamePort {
    std::mutex mutex;
    std::deque<std::promise<amity::GameResponse>> promises;

    std::future<amity::GameResponse> submit(amity::GameRequest) override {
        std::lock_guard<std::mutex> lock(mutex);
        promises.emplace_back();
        return promises.back().get_future();
    }
};

struct RefusingGamePort : amity::GamePort {
    std::atomic<bool> touched{false};

    std::future<amity::GameResponse> submit(amity::GameRequest) override {
        touched = true;
        std::promise<amity::GameResponse> promise;
        promise.set_value(amity::GameResponse{});
        return promise.get_future();
    }
};

class TestClient {
public:
    explicit TestClient(int port) {
        ws_.setUrl("ws://127.0.0.1:" + std::to_string(port) + "/");
        ws_.disableAutomaticReconnection();
        ws_.setOnMessageCallback([this](const ix::WebSocketMessagePtr& msg) {
            std::lock_guard<std::mutex> lock(mutex_);
            if (msg->type == ix::WebSocketMessageType::Open) {
                open_ = true;
            } else if (msg->type == ix::WebSocketMessageType::Close) {
                closed_ = true;
            } else if (msg->type == ix::WebSocketMessageType::Message) {
                received_.push_back(msg->str);
            }
            cv_.notify_all();
        });
        ws_.start();
    }

    ~TestClient() { ws_.stop(); }

    bool wait_open(std::chrono::milliseconds timeout = std::chrono::seconds(5)) {
        std::unique_lock<std::mutex> lock(mutex_);
        cv_.wait_for(lock, timeout, [this] { return open_ || closed_; });
        return open_;
    }

    bool wait_closed(std::chrono::milliseconds timeout = std::chrono::seconds(5)) {
        std::unique_lock<std::mutex> lock(mutex_);
        return cv_.wait_for(lock, timeout, [this] { return closed_; });
    }

    void send(const std::string& text) { ws_.send(text); }

    std::optional<std::string> wait_message(std::chrono::milliseconds timeout = std::chrono::seconds(5)) {
        std::unique_lock<std::mutex> lock(mutex_);
        cv_.wait_for(lock, timeout, [this] { return !received_.empty() || closed_; });
        if (received_.empty()) {
            return std::nullopt;
        }
        std::string msg = received_.front();
        received_.pop_front();
        return msg;
    }

private:
    ix::WebSocket ws_;
    std::mutex mutex_;
    std::condition_variable cv_;
    bool open_ = false;
    bool closed_ = false;
    std::deque<std::string> received_;
};

amity::Envelope must_parse(const std::optional<std::string>& text) {
    REQUIRE(text.has_value());
    std::string err;
    auto env = amity::parse_envelope(*text, err);
    REQUIRE(env.has_value());
    return *env;
}

void handshake(TestClient& client, const std::string& token) {
    REQUIRE(client.wait_open());
    client.send(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    auto helloReply = must_parse(client.wait_message());
    REQUIRE(helloReply.type == "hello_ok");
    std::string nonce = helloReply.data["nonce"].get<std::string>();

    std::string proof = amity::hmac_sha256_hex(token, nonce);
    client.send(R"({"id":"2","type":"auth","data":{"proof":")" + proof + R"("}})");
    auto authReply = must_parse(client.wait_message());
    REQUIRE(authReply.type == "auth_ok");

    auto push = must_parse(client.wait_message());
    REQUIRE(push.id == "push");
    REQUIRE(push.type == "capabilities");
}

}

TEST_CASE("full handshake then get_status round-trip") {
    StubGamePort stub;
    stub.canned_data = nlohmann::json{{"players", 7}};
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    cfg.hello_info = nlohmann::json{{"mod", "psp-amity"}, {"version", "0.1.0"}};
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));
    CHECK(server.port() != 0);

    TestClient client(server.port());
    REQUIRE(client.wait_open());
    client.send(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    auto helloReply = must_parse(client.wait_message());
    CHECK(helloReply.type == "hello_ok");
    CHECK(helloReply.data["mod"] == "psp-amity");
    CHECK(helloReply.data["version"] == "0.1.0");
    std::string nonce = helloReply.data["nonce"].get<std::string>();

    std::string proof = amity::hmac_sha256_hex("s3cr3t", nonce);
    client.send(R"({"id":"2","type":"auth","data":{"proof":")" + proof + R"("}})");
    auto authReply = must_parse(client.wait_message());
    CHECK(authReply.type == "auth_ok");

    auto pushReply = must_parse(client.wait_message());
    CHECK(pushReply.id == "push");
    CHECK(pushReply.type == "capabilities");

    client.send(R"({"id":"3","type":"get_status","data":{}})");
    auto statusReply = must_parse(client.wait_message());
    CHECK(statusReply.id == "3");
    CHECK(statusReply.type == "status");
    CHECK(statusReply.data["players"] == 7);
    CHECK(stub.last_request.op == "status");

    server.stop();
}

TEST_CASE("bind localhost normalizes to 127.0.0.1 and round-trips") {
    StubGamePort stub;
    stub.canned_data = nlohmann::json{{"players", 2}};
    amity::ServerConfig cfg;
    cfg.bind = "localhost";
    cfg.token = "s3cr3t";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));
    CHECK(server.port() != 0);

    TestClient client(server.port());
    handshake(client, "s3cr3t");

    client.send(R"({"id":"3","type":"get_status","data":{}})");
    auto statusReply = must_parse(client.wait_message());
    CHECK(statusReply.type == "status");
    CHECK(statusReply.data["players"] == 2);

    server.stop();
}

TEST_CASE("wrong token exhausts attempts and closes the connection") {
    StubGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    REQUIRE(client.wait_open());
    client.send(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    auto helloReply = must_parse(client.wait_message());
    std::string nonce = helloReply.data["nonce"].get<std::string>();
    std::string wrongProof = amity::hmac_sha256_hex("wrong", nonce);

    for (int i = 0; i < 2; ++i) {
        client.send(R"({"id":"2","type":"auth","data":{"proof":")" + wrongProof + R"("}})");
        auto reply = must_parse(client.wait_message());
        CHECK(reply.type == "error");
        CHECK(reply.data["code"] == "unauthorized");
    }
    client.send(R"({"id":"2","type":"auth","data":{"proof":")" + wrongProof + R"("}})");
    auto lastReply = must_parse(client.wait_message());
    CHECK(lastReply.data["code"] == "unauthorized");
    CHECK(client.wait_closed());

    server.stop();
}

TEST_CASE("unauthenticated request is rejected and closes the connection") {
    StubGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    REQUIRE(client.wait_open());
    client.send(R"({"id":"1","type":"get_status","data":{}})");
    auto reply = must_parse(client.wait_message());
    CHECK(reply.type == "error");
    CHECK(reply.data["code"] == "unauthorized");
    CHECK(client.wait_closed());
    CHECK_FALSE(stub.last_request.op == "status");

    server.stop();
}

TEST_CASE("ephemeral port is nonzero") {
    StubGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));
    CHECK(server.port() > 0);

    server.stop();
}

TEST_CASE("stop() with a connected client returns cleanly") {
    StubGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    REQUIRE(client.wait_open());
    client.send(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    must_parse(client.wait_message());

    server.stop();
    server.stop();
    CHECK(client.wait_closed());
}

TEST_CASE("a request the game port never fulfills times out") {
    NeverFulfillingGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    cfg.request_wait = std::chrono::milliseconds(100);
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    handshake(client, "s3cr3t");

    client.send(R"({"id":"3","type":"get_status","data":{}})");
    auto reply = must_parse(client.wait_message());
    CHECK(reply.id == "3");
    CHECK(reply.type == "error");
    CHECK(reply.data["code"] == "timeout");

    server.stop();
}

TEST_CASE("a failing game response is passed through as an error") {
    StubGamePort stub;
    stub.failure_code = "not_found";
    stub.failure_message = "no such pal";
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    handshake(client, "s3cr3t");

    client.send(R"({"id":"3","type":"get_pal_detail","data":{"id":"x"}})");
    auto reply = must_parse(client.wait_message());
    CHECK(reply.type == "error");
    CHECK(reply.data["code"] == "not_found");
    CHECK(reply.data["message"] == "no such pal");
    CHECK(stub.last_request.op == "pal_detail");

    server.stop();
}

TEST_CASE("bind ::1 fails before listening with an IPv4 error") {
    StubGamePort stub;
    amity::ServerConfig cfg;
    cfg.bind = "::1";
    cfg.token = "s3cr3t";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    CHECK_FALSE(server.start(error));
    CHECK(error.find("IPv4") != std::string::npos);
    CHECK(server.port() == 0);
}

TEST_CASE("empty token fails start") {
    StubGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    CHECK_FALSE(server.start(error));
    CHECK_FALSE(error.empty());
}

TEST_CASE("capabilities push follows auth_ok with the configured snapshot") {
    StubGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    cfg.capabilities_provider = [] {
        return nlohmann::json{
            {"version", 2},
            {"ops", nlohmann::json{{"player.heal", nlohmann::json{{"available", true}, {"reason", nullptr}}}}},
        };
    };
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    REQUIRE(client.wait_open());
    client.send(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    auto helloReply = must_parse(client.wait_message());
    std::string nonce = helloReply.data["nonce"].get<std::string>();

    std::string proof = amity::hmac_sha256_hex("s3cr3t", nonce);
    client.send(R"({"id":"2","type":"auth","data":{"proof":")" + proof + R"("}})");
    auto authReply = must_parse(client.wait_message());
    CHECK(authReply.type == "auth_ok");

    auto push = must_parse(client.wait_message());
    CHECK(push.id == "push");
    CHECK(push.type == "capabilities");
    CHECK(push.data["version"] == 2);
    CHECK(push.data["ops"]["player.heal"]["available"] == true);

    server.stop();
}

TEST_CASE("get_capabilities is answered inline without touching the queue") {
    RefusingGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    cfg.capabilities_provider = [] {
        return nlohmann::json{{"version", 3}, {"ops", nlohmann::json::object()}};
    };
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    handshake(client, "s3cr3t");

    client.send(R"({"id":"9","type":"get_capabilities","data":{}})");
    auto reply = must_parse(client.wait_message());
    CHECK(reply.id == "9");
    CHECK(reply.type == "capabilities");
    CHECK(reply.data["version"] == 3);
    CHECK_FALSE(stub.touched.load());

    server.stop();
}

TEST_CASE("command before auth is rejected the same way as any other pre-auth request") {
    StubGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    REQUIRE(client.wait_open());
    client.send(R"({"id":"1","type":"command","data":{"commandId":"c1","op":"player.heal","args":{}}})");
    auto reply = must_parse(client.wait_message());
    CHECK(reply.type == "error");
    CHECK(reply.data["code"] == "unauthorized");
    CHECK(client.wait_closed());
    CHECK_FALSE(stub.last_request.op == "player.heal");

    server.stop();
}

TEST_CASE("command with malformed data is rejected as validation_failed") {
    StubGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    handshake(client, "s3cr3t");

    client.send(R"({"id":"6","type":"command","data":{"op":"player.heal","args":{}}})");
    auto reply = must_parse(client.wait_message());
    CHECK(reply.type == "error");
    CHECK(reply.data["code"] == "validation_failed");

    server.stop();
}

TEST_CASE("command routes through capability_check and reports capability_unavailable") {
    StubGamePort stub;
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    cfg.capability_check = [](const std::string& op, std::string& reason) {
        if (op == "player.heal") {
            return true;
        }
        reason = "unknown op";
        return false;
    };
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    handshake(client, "s3cr3t");

    client.send(R"({"id":"5","type":"command","data":{"commandId":"c1","op":"unknown.op","args":{}}})");
    auto reply = must_parse(client.wait_message());
    CHECK(reply.type == "error");
    CHECK(reply.data["code"] == "capability_unavailable");
    CHECK_FALSE(stub.last_request.op == "unknown.op");

    server.stop();
}

TEST_CASE("a command round-trips to a command_result reply") {
    StubGamePort stub;
    stub.canned_data = nlohmann::json{
        {"commandId", "c1"}, {"op", "player.heal"},          {"applied", true},
        {"verified", true},  {"retrySafe", true},            {"data", nlohmann::json::object()},
    };
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    amity::BridgeServer server(cfg, stub);

    std::string error;
    REQUIRE(server.start(error));

    TestClient client(server.port());
    handshake(client, "s3cr3t");

    client.send(R"({"id":"7","type":"command","data":{"commandId":"c1","op":"player.heal","args":{"playerUid":"x"}}})");
    auto reply = must_parse(client.wait_message());
    CHECK(reply.id == "7");
    CHECK(reply.type == "command_result");
    CHECK(reply.data["commandId"] == "c1");
    CHECK(reply.data["applied"] == true);
    CHECK(stub.last_request.op == "player.heal");
    CHECK(stub.last_request.args["playerUid"] == "x");

    server.stop();
}

TEST_CASE("start binds a configured port instead of an ephemeral one") {
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    cfg.port = 47913;
    StubGamePort port;
    amity::BridgeServer server(std::move(cfg), port);

    std::string error;
    REQUIRE_MESSAGE(server.start(error), error);
    CHECK(server.port() == 47913);
    server.stop();
}

TEST_CASE("start accepts a non-loopback bind when a token is set") {
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    cfg.bind = "0.0.0.0";
    StubGamePort port;
    amity::BridgeServer server(std::move(cfg), port);

    std::string error;
    REQUIRE_MESSAGE(server.start(error), error);
    CHECK(server.port() != 0);
    server.stop();
}

TEST_CASE("start rejects a bind that is not an IPv4 address") {
    amity::ServerConfig cfg;
    cfg.token = "s3cr3t";
    cfg.bind = "example.com";
    StubGamePort port;
    amity::BridgeServer server(std::move(cfg), port);

    std::string error;
    CHECK_FALSE(server.start(error));
    CHECK(error.find("bind") != std::string::npos);
}

TEST_CASE("a configured port that is already taken fails loudly") {
    StubGamePort port_a;
    amity::ServerConfig first;
    first.token = "s3cr3t";
    first.port = 47914;
    amity::BridgeServer a(std::move(first), port_a);
    std::string error;
    REQUIRE_MESSAGE(a.start(error), error);

    StubGamePort port_b;
    amity::ServerConfig second;
    second.token = "s3cr3t";
    second.port = 47914;
    amity::BridgeServer b(std::move(second), port_b);
    CHECK_FALSE(b.start(error));
    CHECK_FALSE(error.empty());

    a.stop();
}
