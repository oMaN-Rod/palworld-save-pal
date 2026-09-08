#include <doctest/doctest.h>
#include <amity/capability_registry.hpp>

TEST_CASE("snapshot is empty with version zero before any set()") {
    amity::CapabilityRegistry registry;
    auto snapshot = registry.snapshot();
    CHECK(snapshot["version"] == 0);
    CHECK(snapshot["ops"].is_object());
    CHECK(snapshot["ops"].empty());
}

TEST_CASE("set() records availability and a null reason when available") {
    amity::CapabilityRegistry registry;
    registry.set("player.heal", true, "");

    auto snapshot = registry.snapshot();
    CHECK(snapshot["version"] == 1);
    REQUIRE(snapshot["ops"].contains("player.heal"));
    CHECK(snapshot["ops"]["player.heal"]["available"] == true);
    CHECK(snapshot["ops"]["player.heal"]["reason"].is_null());
}

TEST_CASE("set() records the reason string when unavailable") {
    amity::CapabilityRegistry registry;
    registry.set("item.grant", false, "item database not loaded");

    auto snapshot = registry.snapshot();
    REQUIRE(snapshot["ops"].contains("item.grant"));
    CHECK(snapshot["ops"]["item.grant"]["available"] == false);
    CHECK(snapshot["ops"]["item.grant"]["reason"] == "item database not loaded");
}

TEST_CASE("version bumps only when the recorded state actually changes") {
    amity::CapabilityRegistry registry;

    registry.set("player.heal", true, "");
    CHECK(registry.snapshot()["version"] == 1);

    registry.set("player.heal", true, "");
    CHECK(registry.snapshot()["version"] == 1);

    registry.set("player.heal", false, "unresolved symbol");
    CHECK(registry.snapshot()["version"] == 2);

    registry.set("player.heal", false, "unresolved symbol");
    CHECK(registry.snapshot()["version"] == 2);

    registry.set("player.heal", false, "still unresolved");
    CHECK(registry.snapshot()["version"] == 3);
}

TEST_CASE("available() passes through the exact reason that was set") {
    amity::CapabilityRegistry registry;
    registry.set("item.grant", false, "item database not loaded");

    std::string reason;
    CHECK_FALSE(registry.available("item.grant", reason));
    CHECK(reason == "item database not loaded");

    registry.set("item.grant", true, "");
    CHECK(registry.available("item.grant", reason));
    CHECK(reason.empty());
}

TEST_CASE("available() reports unavailable for an op that was never registered") {
    amity::CapabilityRegistry registry;
    std::string reason;
    CHECK_FALSE(registry.available("unknown.op", reason));
}
