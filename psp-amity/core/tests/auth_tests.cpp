#include <doctest/doctest.h>
#include <amity/auth.hpp>
#include <amity/hmac.hpp>
#include <amity/protocol.hpp>

namespace {
const std::string kNonce = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

std::string auth_message(const std::string& id, const std::string& token, const std::string& nonce) {
    return R"({"id":")" + id + R"(","type":"auth","data":{"proof":")" +
           amity::hmac_sha256_hex(token, nonce) + R"("}})";
}
}

TEST_CASE("fixed_time_equals compares content and length") {
    CHECK(amity::fixed_time_equals("secret", "secret"));
    CHECK_FALSE(amity::fixed_time_equals("secret", "secreT"));
    CHECK_FALSE(amity::fixed_time_equals("secret", "secrets"));
    CHECK_FALSE(amity::fixed_time_equals("", "x"));
    CHECK(amity::fixed_time_equals("", ""));
}

TEST_CASE("hello advertises protocol 2 and a nonce") {
    amity::Session session("s3cr3t", kNonce, nlohmann::json{{"mod", "psp-amity"}, {"version", "0.1.0"}});
    auto hello = session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    REQUIRE(hello.send.size() == 1);
    std::string err;
    auto reply = amity::parse_envelope(hello.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->type == "hello_ok");
    CHECK(reply->data["protocolVersion"] == 2);
    CHECK(reply->data["nonce"] == kNonce);
    CHECK(reply->data.contains("mod"));
    CHECK_FALSE(session.ready());
}

TEST_CASE("a valid proof authenticates") {
    amity::Session session("s3cr3t", kNonce, nlohmann::json{{"mod", "psp-amity"}});
    session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    auto auth = session.on_message(auth_message("2", "s3cr3t", kNonce));
    REQUIRE(auth.send.size() == 1);
    CHECK_FALSE(auth.close);
    std::string err;
    auto reply = amity::parse_envelope(auth.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->type == "auth_ok");
    CHECK(session.ready());
}

TEST_CASE("a proof from the wrong token is rejected") {
    amity::Session session("s3cr3t", kNonce, nlohmann::json{{"mod", "psp-amity"}});
    session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    auto auth = session.on_message(auth_message("2", "wrong", kNonce));
    std::string err;
    auto reply = amity::parse_envelope(auth.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->type == "error");
    CHECK(reply->data["code"] == "unauthorized");
    CHECK_FALSE(session.ready());
}

TEST_CASE("a proof captured for another nonce does not replay") {
    const std::string other_nonce(64, 'a');
    amity::Session session("s3cr3t", kNonce, nlohmann::json{{"mod", "psp-amity"}});
    session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    auto auth = session.on_message(auth_message("2", "s3cr3t", other_nonce));
    std::string err;
    auto reply = amity::parse_envelope(auth.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->type == "error");
    CHECK_FALSE(session.ready());
}

TEST_CASE("a cleartext token is not accepted as a proof") {
    amity::Session session("s3cr3t", kNonce, nlohmann::json{{"mod", "psp-amity"}});
    session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    auto auth = session.on_message(R"({"id":"2","type":"auth","data":{"token":"s3cr3t"}})");
    std::string err;
    auto reply = amity::parse_envelope(auth.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->type == "error");
    CHECK_FALSE(session.ready());
}

TEST_CASE("protocol 1 is refused") {
    amity::Session session("s3cr3t", kNonce, nlohmann::json{{"mod", "psp-amity"}});
    auto hello = session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":1}})");
    CHECK(hello.close);
    std::string err;
    auto reply = amity::parse_envelope(hello.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->data["code"] == "protocol_mismatch");
}

TEST_CASE("repeated bad proofs close the connection at the attempt limit") {
    amity::Session session("s3cr3t", kNonce, nlohmann::json{{"mod", "psp-amity"}}, 2);
    session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":2}})");
    auto first = session.on_message(auth_message("2", "wrong", kNonce));
    CHECK_FALSE(first.close);
    auto second = session.on_message(auth_message("3", "wrong", kNonce));
    CHECK(second.close);
}
