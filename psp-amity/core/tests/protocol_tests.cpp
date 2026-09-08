#include <doctest/doctest.h>
#include <amity/protocol.hpp>

TEST_CASE("parse round-trips a valid envelope") {
    std::string err;
    auto e = amity::parse_envelope(R"({"id":"7","type":"hello","data":{"protocolVersion":1}})", err);
    REQUIRE(e.has_value());
    CHECK(e->id == "7");
    CHECK(e->type == "hello");
    CHECK(e->data["protocolVersion"] == 1);
    auto text = amity::serialize_envelope(*e);
    auto e2 = amity::parse_envelope(text, err);
    REQUIRE(e2.has_value());
    CHECK(e2->type == "hello");
}

TEST_CASE("parse rejects junk, missing fields, and non-object data") {
    std::string err;
    CHECK_FALSE(amity::parse_envelope("not json", err).has_value());
    CHECK_FALSE(amity::parse_envelope(R"({"type":"x"})", err).has_value());
    CHECK_FALSE(amity::parse_envelope(R"({"id":"1","type":"x","data":3})", err).has_value());
}

TEST_CASE("make_error shape") {
    std::string err;
    auto e = amity::parse_envelope(amity::make_error("9", "bad_request", "nope"), err);
    REQUIRE(e.has_value());
    CHECK(e->type == "error");
    CHECK(e->data["code"] == "bad_request");
    CHECK(e->data["message"] == "nope");
}
