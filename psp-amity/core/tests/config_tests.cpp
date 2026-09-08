#include <doctest/doctest.h>
#include <amity/config.hpp>

#include <map>
#include <string>

namespace {

amity::EnvLookup env_from(std::map<std::string, std::string> values) {
    return [values = std::move(values)](const char* key) -> std::optional<std::string> {
        auto it = values.find(key);
        if (it == values.end()) return std::nullopt;
        return it->second;
    };
}

}

TEST_CASE("parse_ini returns defaults for empty input") {
    auto cfg = amity::parse_ini("");
    CHECK(cfg.name == "PSPAmity");
    CHECK(cfg.port == 0);
    CHECK(cfg.token.empty());
    CHECK(cfg.bind == "127.0.0.1");
}

TEST_CASE("parse_ini reads the bridge section") {
    auto cfg = amity::parse_ini(
        "[bridge]\n"
        "name = Dedicated - psp4\n"
        "port = 8788\n"
        "token = s3cr3t\n"
        "bind = 0.0.0.0\n");
    CHECK(cfg.name == "Dedicated - psp4");
    CHECK(cfg.port == 8788);
    CHECK(cfg.token == "s3cr3t");
    CHECK(cfg.bind == "0.0.0.0");
}

TEST_CASE("parse_ini ignores comments, blanks and stray whitespace") {
    auto cfg = amity::parse_ini(
        "; leading comment\n"
        "\n"
        "  [bridge]  \r\n"
        "# hash comment\n"
        "   port   =   9001   \r\n"
        "name=Solo World\n");
    CHECK(cfg.port == 9001);
    CHECK(cfg.name == "Solo World");
}

TEST_CASE("parse_ini matches keys case-insensitively") {
    auto cfg = amity::parse_ini("[BRIDGE]\nPORT = 7000\nToKeN = abc\n");
    CHECK(cfg.port == 7000);
    CHECK(cfg.token == "abc");
}

TEST_CASE("parse_ini ignores unknown keys, unknown sections and malformed lines") {
    auto cfg = amity::parse_ini(
        "[other]\nport = 1\n"
        "[bridge]\nfuture_key = whatever\nthis line has no equals\nport = 8080\n");
    CHECK(cfg.port == 8080);
}

TEST_CASE("parse_ini leaves the default when a port is not a number") {
    auto cfg = amity::parse_ini("[bridge]\nport = abc\n");
    CHECK(cfg.port == 0);
}

TEST_CASE("apply_env_overrides wins over file values") {
    auto cfg = amity::parse_ini("[bridge]\nport = 8788\nname = From File\n");
    amity::apply_env_overrides(cfg, env_from({{"PSPAMITY_PORT", "9999"},
                                              {"PSPAMITY_NAME", "From Env"},
                                              {"PSPAMITY_TOKEN", "envtoken"},
                                              {"PSPAMITY_BIND", "0.0.0.0"}}));
    CHECK(cfg.port == 9999);
    CHECK(cfg.name == "From Env");
    CHECK(cfg.token == "envtoken");
    CHECK(cfg.bind == "0.0.0.0");
}

TEST_CASE("apply_env_overrides leaves file values when env is unset") {
    auto cfg = amity::parse_ini("[bridge]\nport = 8788\nname = From File\n");
    amity::apply_env_overrides(cfg, env_from({}));
    CHECK(cfg.port == 8788);
    CHECK(cfg.name == "From File");
}

TEST_CASE("is_loopback_bind recognises the loopback spellings") {
    CHECK(amity::is_loopback_bind("127.0.0.1"));
    CHECK(amity::is_loopback_bind("localhost"));
    CHECK_FALSE(amity::is_loopback_bind("0.0.0.0"));
    CHECK_FALSE(amity::is_loopback_bind("192.168.1.20"));
}

TEST_CASE("validate_config accepts the defaults") {
    std::string error;
    CHECK(amity::validate_config(amity::BridgeConfig{}, error));
    CHECK(error.empty());
}

TEST_CASE("validate_config rejects a reserved or out-of-range port") {
    amity::BridgeConfig cfg;
    std::string error;
    cfg.port = 80;
    CHECK_FALSE(amity::validate_config(cfg, error));
    CHECK(error.find("port") != std::string::npos);
    cfg.port = 70000;
    CHECK_FALSE(amity::validate_config(cfg, error));
    cfg.port = 1024;
    CHECK(amity::validate_config(cfg, error));
    cfg.port = 65535;
    CHECK(amity::validate_config(cfg, error));
}

TEST_CASE("validate_config rejects a bind that is not an IPv4 literal") {
    amity::BridgeConfig cfg;
    cfg.token = "s3cr3t";
    cfg.bind = "example.com";
    std::string error;
    CHECK_FALSE(amity::validate_config(cfg, error));
    CHECK(error.find("bind") != std::string::npos);
}

TEST_CASE("validate_config refuses a non-loopback bind without an explicit token") {
    amity::BridgeConfig cfg;
    cfg.bind = "0.0.0.0";
    std::string error;
    CHECK_FALSE(amity::validate_config(cfg, error));
    CHECK(error.find("token") != std::string::npos);

    cfg.token = "s3cr3t";
    CHECK(amity::validate_config(cfg, error));
}

TEST_CASE("validate_config allows an empty token on loopback") {
    amity::BridgeConfig cfg;
    cfg.bind = "127.0.0.1";
    cfg.token.clear();
    std::string error;
    CHECK(amity::validate_config(cfg, error));
}
