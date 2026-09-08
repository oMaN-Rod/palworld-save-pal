#include <doctest/doctest.h>
#include <amity/auth.hpp>
#include <amity/protocol.hpp>

TEST_CASE("fixed_time_equals compares content and length") {
    CHECK(amity::fixed_time_equals("secret", "secret"));
    CHECK_FALSE(amity::fixed_time_equals("secret", "secreT"));
    CHECK_FALSE(amity::fixed_time_equals("secret", "secrets"));
    CHECK_FALSE(amity::fixed_time_equals("", "x"));
    CHECK(amity::fixed_time_equals("", ""));
}

TEST_CASE("hello then auth then request passes through") {
    amity::Session session("s3cr3t", nlohmann::json{{"mod", "psp-amity"}, {"version", "0.1.0"}});
    CHECK_FALSE(session.ready());

    auto hello = session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":1}})");
    REQUIRE(hello.send.size() == 1);
    CHECK_FALSE(hello.close);
    CHECK_FALSE(hello.request.has_value());
    std::string err;
    auto helloReply = amity::parse_envelope(hello.send[0], err);
    REQUIRE(helloReply.has_value());
    CHECK(helloReply->type == "hello_ok");
    CHECK(helloReply->data["protocolVersion"] == amity::PROTOCOL_VERSION);
    CHECK(helloReply->data["mod"] == "psp-amity");
    CHECK(helloReply->data["version"] == "0.1.0");
    CHECK_FALSE(session.ready());

    auto auth = session.on_message(R"({"id":"2","type":"auth","data":{"token":"s3cr3t"}})");
    REQUIRE(auth.send.size() == 1);
    CHECK_FALSE(auth.close);
    auto authReply = amity::parse_envelope(auth.send[0], err);
    REQUIRE(authReply.has_value());
    CHECK(authReply->type == "auth_ok");
    CHECK(authReply->data.empty());
    CHECK(session.ready());

    auto req = session.on_message(R"({"id":"3","type":"do_thing","data":{"x":1}})");
    CHECK(req.send.empty());
    CHECK_FALSE(req.close);
    REQUIRE(req.request.has_value());
    CHECK(req.request->id == "3");
    CHECK(req.request->type == "do_thing");
    CHECK(req.request->data["x"] == 1);
}

TEST_CASE("wrong token exhausts attempts and closes") {
    amity::Session session("s3cr3t", nlohmann::json::object(), 3);
    session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":1}})");

    std::string err;
    for (int i = 0; i < 2; ++i) {
        auto out = session.on_message(R"({"id":"2","type":"auth","data":{"token":"wrong"}})");
        REQUIRE(out.send.size() == 1);
        CHECK_FALSE(out.close);
        auto reply = amity::parse_envelope(out.send[0], err);
        REQUIRE(reply.has_value());
        CHECK(reply->type == "error");
        CHECK(reply->data["code"] == "unauthorized");
        CHECK_FALSE(session.ready());
    }
    auto last = session.on_message(R"({"id":"2","type":"auth","data":{"token":"wrong"}})");
    REQUIRE(last.send.size() == 1);
    CHECK(last.close);
    auto reply = amity::parse_envelope(last.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->data["code"] == "unauthorized");
    CHECK_FALSE(session.ready());
}

TEST_CASE("closed latch survives a correct token sent after lockout") {
    amity::Session session("s3cr3t", nlohmann::json::object(), 3);
    session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":1}})");
    session.on_message(R"({"id":"2","type":"auth","data":{"token":"wrong"}})");
    session.on_message(R"({"id":"3","type":"auth","data":{"token":"wrong"}})");
    auto lockout = session.on_message(R"({"id":"4","type":"auth","data":{"token":"wrong"}})");
    REQUIRE(lockout.close);
    CHECK_FALSE(session.ready());

    auto afterLockout = session.on_message(R"({"id":"5","type":"auth","data":{"token":"s3cr3t"}})");
    CHECK(afterLockout.send.empty());
    CHECK(afterLockout.close);
    CHECK_FALSE(afterLockout.request.has_value());
    CHECK_FALSE(session.ready());
}

TEST_CASE("closed latch survives a valid hello sent after a protocol_mismatch close") {
    amity::Session session("s3cr3t", nlohmann::json::object());
    auto mismatch = session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":99}})");
    REQUIRE(mismatch.close);
    CHECK_FALSE(session.ready());

    auto afterClose = session.on_message(R"({"id":"2","type":"hello","data":{"protocolVersion":1}})");
    CHECK(afterClose.send.empty());
    CHECK(afterClose.close);
    CHECK_FALSE(afterClose.request.has_value());
    CHECK_FALSE(session.ready());

    auto authAttempt = session.on_message(R"({"id":"3","type":"auth","data":{"token":"s3cr3t"}})");
    CHECK(authAttempt.send.empty());
    CHECK(authAttempt.close);
    CHECK_FALSE(session.ready());
}

TEST_CASE("well-formed non-protocol request before auth is unauthorized and closes") {
    std::string err;

    amity::Session beforeHello("s3cr3t", nlohmann::json::object());
    auto out1 = beforeHello.on_message(R"({"id":"1","type":"do_thing","data":{}})");
    REQUIRE(out1.send.size() == 1);
    CHECK(out1.close);
    CHECK_FALSE(out1.request.has_value());
    auto reply1 = amity::parse_envelope(out1.send[0], err);
    REQUIRE(reply1.has_value());
    CHECK(reply1->type == "error");
    CHECK(reply1->data["code"] == "unauthorized");

    amity::Session afterHello("s3cr3t", nlohmann::json::object());
    afterHello.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":1}})");
    auto out2 = afterHello.on_message(R"({"id":"2","type":"do_thing","data":{}})");
    REQUIRE(out2.send.size() == 1);
    CHECK(out2.close);
    CHECK_FALSE(out2.request.has_value());
    auto reply2 = amity::parse_envelope(out2.send[0], err);
    REQUIRE(reply2.has_value());
    CHECK(reply2->data["code"] == "unauthorized");
}

TEST_CASE("hello twice is bad_request and closes") {
    amity::Session session("s3cr3t", nlohmann::json::object());
    session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":1}})");
    auto out = session.on_message(R"({"id":"2","type":"hello","data":{"protocolVersion":1}})");
    REQUIRE(out.send.size() == 1);
    CHECK(out.close);
    std::string err;
    auto reply = amity::parse_envelope(out.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->data["code"] == "bad_request");
    CHECK_FALSE(session.ready());
}

TEST_CASE("auth before hello is bad_request and closes") {
    amity::Session session("s3cr3t", nlohmann::json::object());
    auto out = session.on_message(R"({"id":"1","type":"auth","data":{"token":"s3cr3t"}})");
    REQUIRE(out.send.size() == 1);
    CHECK(out.close);
    std::string err;
    auto reply = amity::parse_envelope(out.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->data["code"] == "bad_request");
    CHECK_FALSE(session.ready());
}

TEST_CASE("wrong protocol version is protocol_mismatch and closes") {
    amity::Session session("s3cr3t", nlohmann::json::object());
    auto out = session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":99}})");
    REQUIRE(out.send.size() == 1);
    CHECK(out.close);
    std::string err;
    auto reply = amity::parse_envelope(out.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->data["code"] == "protocol_mismatch");
    CHECK_FALSE(session.ready());
}

TEST_CASE("malformed json before ready is bad_request and closes") {
    amity::Session session("s3cr3t", nlohmann::json::object());
    auto out = session.on_message("not json");
    REQUIRE(out.send.size() == 1);
    CHECK(out.close);
    std::string err;
    auto reply = amity::parse_envelope(out.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->data["code"] == "bad_request");
}

TEST_CASE("malformed json after ready is bad_request without closing") {
    amity::Session session("s3cr3t", nlohmann::json::object());
    session.on_message(R"({"id":"1","type":"hello","data":{"protocolVersion":1}})");
    session.on_message(R"({"id":"2","type":"auth","data":{"token":"s3cr3t"}})");
    REQUIRE(session.ready());

    auto out = session.on_message("not json");
    REQUIRE(out.send.size() == 1);
    CHECK_FALSE(out.close);
    CHECK_FALSE(out.request.has_value());
    std::string err;
    auto reply = amity::parse_envelope(out.send[0], err);
    REQUIRE(reply.has_value());
    CHECK(reply->data["code"] == "bad_request");
    CHECK(session.ready());
}
