#include <doctest/doctest.h>
#include <amity/endpoint_file.hpp>
#include <amity/token.hpp>

#include <nlohmann/json.hpp>
#include <windows.h>
#include <algorithm>
#include <cctype>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <system_error>

namespace {

struct TempDirGuard {
    std::filesystem::path path;

    explicit TempDirGuard(std::filesystem::path p) : path(std::move(p)) {
        std::filesystem::create_directories(path);
    }

    ~TempDirGuard() {
        std::error_code ec;
        std::filesystem::remove_all(path, ec);
    }

    TempDirGuard(const TempDirGuard&) = delete;
    TempDirGuard& operator=(const TempDirGuard&) = delete;
};

std::filesystem::path unique_temp_path() {
    return std::filesystem::temp_directory_path() / ("amity_endpoint_test_" + amity::generate_token_hex());
}

std::string read_file(const std::filesystem::path& path) {
    std::ifstream in(path, std::ios::binary);
    std::ostringstream ss;
    ss << in.rdbuf();
    return ss.str();
}

}

TEST_CASE("generate_token_hex produces 64 lowercase hex characters") {
    std::string token = amity::generate_token_hex();
    CHECK(token.size() == 64);
    CHECK(std::all_of(token.begin(), token.end(), [](unsigned char c) {
        return std::isdigit(c) || (c >= 'a' && c <= 'f');
    }));
}

TEST_CASE("generate_token_hex differs between calls") {
    std::string a = amity::generate_token_hex();
    std::string b = amity::generate_token_hex();
    CHECK(a != b);
}

TEST_CASE("write_endpoint_file writes valid JSON with all five keys and this process's pid") {
    TempDirGuard guard(unique_temp_path());
    std::string error;
    bool ok = amity::write_endpoint_file(guard.path, 12345, "sometoken", error);
    REQUIRE(ok);
    CHECK(error.empty());

    auto content = read_file(guard.path / "endpoint.json");
    auto j = nlohmann::json::parse(content);

    CHECK(j["protocolVersion"] == 1);
    CHECK(j["port"] == 12345);
    CHECK(j["token"] == "sometoken");
    CHECK(j["pid"] == static_cast<int>(GetCurrentProcessId()));
    REQUIRE(j.contains("startedAt"));
    CHECK(j["startedAt"].is_string());
}

TEST_CASE("write_endpoint_file leaves no .tmp sibling after a successful write") {
    TempDirGuard guard(unique_temp_path());
    std::string error;
    bool ok = amity::write_endpoint_file(guard.path, 1, "t", error);
    REQUIRE(ok);

    bool found_tmp = false;
    for (auto const& entry : std::filesystem::directory_iterator(guard.path)) {
        if (entry.path().extension() == ".tmp") {
            found_tmp = true;
        }
    }
    CHECK_FALSE(found_tmp);
}

TEST_CASE("write_endpoint_file creates a nonexistent nested directory") {
    TempDirGuard guard(unique_temp_path());
    auto dir = guard.path / "nested" / "more";
    std::string error;
    bool ok = amity::write_endpoint_file(dir, 1, "t", error);
    REQUIRE(ok);
    CHECK(std::filesystem::exists(dir / "endpoint.json"));
}

TEST_CASE("remove_endpoint_file deletes the file and a second remove is a no-op") {
    TempDirGuard guard(unique_temp_path());
    std::string error;
    REQUIRE(amity::write_endpoint_file(guard.path, 1, "t", error));
    REQUIRE(std::filesystem::exists(guard.path / "endpoint.json"));

    amity::remove_endpoint_file(guard.path);
    CHECK_FALSE(std::filesystem::exists(guard.path / "endpoint.json"));

    amity::remove_endpoint_file(guard.path);
    CHECK_FALSE(std::filesystem::exists(guard.path / "endpoint.json"));
}
