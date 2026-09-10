#include <doctest/doctest.h>
#include <amity/protocol.hpp>

#include <filesystem>
#include <fstream>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

namespace {

std::string read_file(const std::string& name) {
    std::filesystem::path path = std::filesystem::path(AMITY_FIXTURES_DIR) / name;
    std::ifstream in(path, std::ios::binary);
    std::ostringstream ss;
    ss << in.rdbuf();
    return ss.str();
}

amity::Envelope load_envelope(const std::string& name) {
    std::string text = read_file(name);
    std::string err;
    auto e = amity::parse_envelope(text, err);
    REQUIRE_MESSAGE(e.has_value(), name, ": ", err);
    return *e;
}

std::string reply_type_for(const std::string& request_type) {
    if (request_type == "hello") return "hello_ok";
    if (request_type == "auth") return "auth_ok";
    if (request_type == "command") return "command_result";
    const std::string prefix = "get_";
    if (request_type.size() > prefix.size() && request_type.rfind(prefix, 0) == 0) {
        return request_type.substr(prefix.size());
    }
    return request_type;
}

const std::vector<std::string> kFixtureFiles = {
    "hello.json",
    "hello_ok.json",
    "auth.json",
    "auth_ok.json",
    "get_status.json",
    "status.json",
    "get_players.json",
    "players.json",
    "get_pals.json",
    "pals.json",
    "get_pal_detail.json",
    "pal_detail.json",
    "get_inventory.json",
    "inventory.json",
    "error_unauthorized.json",
    "error_capability_unavailable.json",
    "error_timeout.json",
    "command_heal.json",
    "command_result_heal.json",
    "command_set_item_slot.json",
    "command_result_set_item_slot.json",
    "get_capabilities.json",
    "capabilities.json",
    "error_not_authoritative.json",
    "error_queue_full.json",
    "error_validation_failed.json",
    "error_game_error.json",
    "error_shutting_down.json",
};

const std::vector<std::pair<std::string, std::string>> kRequestReplyPairs = {
    {"hello.json", "hello_ok.json"},
    {"auth.json", "auth_ok.json"},
    {"get_status.json", "status.json"},
    {"get_players.json", "players.json"},
    {"get_pals.json", "pals.json"},
    {"get_pal_detail.json", "pal_detail.json"},
    {"get_inventory.json", "inventory.json"},
    {"command_heal.json", "command_result_heal.json"},
    {"command_set_item_slot.json", "command_result_set_item_slot.json"},
    {"get_capabilities.json", "capabilities.json"},
};

}

TEST_CASE("every fixture parses as a complete envelope with non-empty id and type") {
    for (const auto& name : kFixtureFiles) {
        CAPTURE(name);
        auto e = load_envelope(name);
        CHECK_FALSE(e.id.empty());
        CHECK_FALSE(e.type.empty());
    }
}

TEST_CASE("every fixture round-trips through serialize_envelope and parse_envelope") {
    for (const auto& name : kFixtureFiles) {
        CAPTURE(name);
        auto e = load_envelope(name);
        std::string text = amity::serialize_envelope(e);
        std::string err;
        auto e2 = amity::parse_envelope(text, err);
        REQUIRE(e2.has_value());
        CHECK(e2->id == e.id);
        CHECK(e2->type == e.type);
        CHECK(e2->data == e.data);
    }
}

TEST_CASE("request fixtures map to their reply fixture's type via the get_ prefix rule") {
    for (const auto& [request_name, reply_name] : kRequestReplyPairs) {
        CAPTURE(request_name);
        auto request = load_envelope(request_name);
        auto reply = load_envelope(reply_name);
        CHECK(reply_type_for(request.type) == reply.type);
        CHECK(request.id == reply.id);
    }
}

TEST_CASE("status.json data carries the six status keys") {
    auto e = load_envelope("status.json");
    for (const std::string key :
         {"authoritative", "modVersion", "mode", "protocolVersion", "queueDepth", "worldLoaded"}) {
        CAPTURE(key);
        CHECK(e.data.contains(key));
    }
    CHECK(e.data.size() == 6);
    CHECK(e.data["protocolVersion"] == amity::PROTOCOL_VERSION);
}

TEST_CASE("hello.json and hello_ok.json describe the current protocol version") {
    auto hello = load_envelope("hello.json");
    CHECK(hello.data["protocolVersion"] == amity::PROTOCOL_VERSION);

    auto hello_ok = load_envelope("hello_ok.json");
    for (const std::string key : {"mod", "protocolVersion", "version", "name", "nonce"}) {
        CAPTURE(key);
        CHECK(hello_ok.data.contains(key));
    }
    CHECK(hello_ok.data["protocolVersion"] == amity::PROTOCOL_VERSION);
    CHECK(hello_ok.data["nonce"].get<std::string>().size() == 64);
}

TEST_CASE("auth.json carries a proof, never a cleartext token") {
    auto e = load_envelope("auth.json");
    CHECK(e.data.contains("proof"));
    CHECK_FALSE(e.data.contains("token"));
    CHECK(e.data["proof"].get<std::string>().size() == 64);
}

TEST_CASE("players.json players[0] has the ten player keys") {
    auto e = load_envelope("players.json");
    REQUIRE(e.data.contains("players"));
    REQUIRE(e.data["players"].is_array());
    REQUIRE_FALSE(e.data["players"].empty());
    const auto& player = e.data["players"][0];
    for (const std::string key :
         {"exp", "guildId", "level", "nickname", "status", "uid", "x", "y", "z", "yaw"}) {
        CAPTURE(key);
        CHECK(player.contains(key));
    }
    CHECK(player.size() == 10);
}

TEST_CASE("pals.json carries the page, its container, the party and the pals") {
    auto e = load_envelope("pals.json");
    for (const std::string key :
         {"page", "pageCount", "slotCount", "slotBase", "containerId", "party", "partyStatus", "status", "pals"}) {
        CAPTURE(key);
        CHECK(e.data.contains(key));
    }
    CHECK(e.data.size() == 9);
    REQUIRE(e.data["pals"].is_array());
    CHECK(e.data["pals"].size() == 2);
    CHECK(e.data["party"].is_array());
    static const std::vector<std::string> summary_keys = {
        "instanceId", "playerUid", "characterId", "nickname", "level", "gender", "isLucky", "ownerUid",
        "slotIndex", "isSick", "isFainted", "isAwakened", "hp", "maxHp",
    };
    for (const auto& pal : e.data["pals"]) {
        for (const auto& key : summary_keys) {
            CAPTURE(key);
            CHECK(pal.contains(key));
        }
        CHECK(pal.size() == summary_keys.size());
    }
}

TEST_CASE("get_pal_detail.json addresses a pal by slot, with no page") {
    auto e = load_envelope("get_pal_detail.json");
    CHECK(e.data.contains("playerUid"));
    CHECK(e.data.contains("slotIndex"));
    CHECK_FALSE(e.data.contains("page"));
}

TEST_CASE("command_heal.json addresses a pal by its storage-wide slot index alone") {
    auto e = load_envelope("command_heal.json");
    CHECK(e.data["args"].contains("playerUid"));
    CHECK(e.data["args"].contains("slotIndex"));
    CHECK(e.data["args"].size() == 2);
}

TEST_CASE("pal_detail.json data has all detail keys") {
    auto e = load_envelope("pal_detail.json");
    static const std::vector<std::string> keys = {
        "instanceId", "playerUid",   "characterId", "nickname",      "level",          "gender",
        "isLucky",    "ownerUid",    "slotIndex",   "exp",           "rank",           "rankHp",
        "rankAttack", "rankDefense", "rankCraftSpeed", "talentHp",   "talentShot",     "talentDefense",
        "passiveSkills", "equipWaza", "masteredWaza", "hp",          "status",         "isSick",
        "isFainted",  "isAwakened",  "maxHp",       "workSuitability", "sanity",       "stomach",
        "maxStomach", "friendshipPoint",
    };
    for (const auto& key : keys) {
        CAPTURE(key);
        CHECK(e.data.contains(key));
    }
    CHECK(e.data.size() == keys.size());
}

TEST_CASE("inventory.json carries playerUid, status, and the five containers in order") {
    auto e = load_envelope("inventory.json");
    CHECK(e.data.contains("playerUid"));
    CHECK(e.data.contains("status"));
    REQUIRE(e.data.contains("containers"));
    CHECK(e.data.size() == 3);

    const auto& containers = e.data["containers"];
    REQUIRE(containers.is_array());
    REQUIRE(containers.size() == 5);
    static const std::vector<std::string> kTypes = {
        "common", "essential", "weaponLoadout", "playerEquipArmor", "foodEquip",
    };
    for (std::size_t i = 0; i < containers.size(); ++i) {
        CAPTURE(i);
        const auto& container = containers[i];
        CHECK(container["type"] == kTypes[i]);
        for (const std::string key : {"containerId", "slotNum", "slots", "status", "type"}) {
            CAPTURE(key);
            CHECK(container.contains(key));
        }
        CHECK(container.size() == 5);
        CHECK(container["slots"].is_array());
    }
}

TEST_CASE("inventory.json is sparse: every emitted slot names a real item") {
    auto e = load_envelope("inventory.json");
    std::size_t emitted = 0;
    for (const auto& container : e.data["containers"]) {
        CHECK(container["slots"].size() <= container["slotNum"].get<std::size_t>());
        for (const auto& slot : container["slots"]) {
            for (const std::string key : {"count", "dynamicItem", "slotIndex", "staticItemId"}) {
                CAPTURE(key);
                CHECK(slot.contains(key));
            }
            CHECK(slot.size() == 4);
            CHECK(slot["staticItemId"].is_string());
            CHECK_FALSE(slot["staticItemId"].get<std::string>().empty());
            CHECK(slot["staticItemId"] != "None");
            ++emitted;
        }
    }
    CHECK(emitted == 4);
}

TEST_CASE("inventory.json dynamic items carry one key set across weapon, armor, and egg") {
    auto e = load_envelope("inventory.json");
    static const std::vector<std::string> keys = {
        "characterId", "durability", "localId", "maxDurability", "passiveSkills", "remainingBullets", "type",
    };
    std::vector<std::string> seen;
    for (const auto& container : e.data["containers"]) {
        for (const auto& slot : container["slots"]) {
            if (slot["dynamicItem"].is_null()) {
                continue;
            }
            const auto& dynamic_item = slot["dynamicItem"];
            for (const auto& key : keys) {
                CAPTURE(key);
                CHECK(dynamic_item.contains(key));
            }
            CHECK(dynamic_item.size() == keys.size());
            CHECK(dynamic_item["localId"].is_string());
            CHECK(dynamic_item["passiveSkills"].is_array());
            seen.push_back(dynamic_item["type"].get<std::string>());
        }
    }
    CHECK(seen == std::vector<std::string>{"weapon", "egg", "armor"});
}

TEST_CASE("inventory.json sends no item metadata the desktop page resolves itself") {
    auto e = load_envelope("inventory.json");
    for (const auto& container : e.data["containers"]) {
        for (const auto& slot : container["slots"]) {
            for (const std::string key :
                 {"icon", "rarity", "weight", "typeA", "typeB", "maxStack", "itemGroup"}) {
                CAPTURE(key);
                CHECK_FALSE(slot.contains(key));
            }
        }
    }
}

TEST_CASE("error fixtures carry code and message") {
    for (const auto& name : {
             "error_unauthorized.json",
             "error_capability_unavailable.json",
             "error_timeout.json",
             "error_not_authoritative.json",
             "error_queue_full.json",
             "error_validation_failed.json",
             "error_game_error.json",
             "error_shutting_down.json",
         }) {
        CAPTURE(name);
        auto e = load_envelope(name);
        CHECK(e.type == "error");
        REQUIRE(e.data.contains("code"));
        REQUIRE(e.data.contains("message"));
        CHECK(e.data["code"].is_string());
        CHECK(e.data["message"].is_string());
    }
}

TEST_CASE("command fixtures carry commandId, op, and args") {
    for (const auto& name : {"command_heal.json", "command_set_item_slot.json"}) {
        CAPTURE(name);
        auto e = load_envelope(name);
        CHECK(e.type == "command");
        CHECK(e.data.contains("commandId"));
        CHECK(e.data.contains("op"));
        CHECK(e.data.contains("args"));
        CHECK(e.data.size() == 3);
    }
}

TEST_CASE("command_result fixtures carry exactly the six result keys") {
    for (const auto& name : {"command_result_heal.json", "command_result_set_item_slot.json"}) {
        CAPTURE(name);
        auto e = load_envelope(name);
        CHECK(e.type == "command_result");
        for (const std::string key : {"commandId", "op", "applied", "verified", "retrySafe", "data"}) {
            CAPTURE(key);
            CHECK(e.data.contains(key));
        }
        CHECK(e.data.size() == 6);
    }
}

TEST_CASE("capabilities.json has version and ops, each op entry has two keys") {
    auto e = load_envelope("capabilities.json");
    CHECK(e.type == "capabilities");
    CHECK(e.data.contains("version"));
    REQUIRE(e.data.contains("ops"));
    CHECK(e.data.size() == 2);
    REQUIRE(e.data["ops"].is_object());
    REQUIRE_FALSE(e.data["ops"].empty());
    for (const auto& [op, entry] : e.data["ops"].items()) {
        CAPTURE(op);
        CHECK(entry.contains("available"));
        CHECK(entry.contains("reason"));
        CHECK(entry.size() == 2);
    }
}
