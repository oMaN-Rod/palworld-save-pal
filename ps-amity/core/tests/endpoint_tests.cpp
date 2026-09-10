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

TEST_CASE("write_endpoint_file writes a per-pid file with the full descriptor") {
    TempDirGuard dir(unique_temp_path());
    std::string error;
    REQUIRE(amity::write_endpoint_file(dir.path, 8788, "s3cr3t", "Solo World", "127.0.0.1", error));

    auto expected = dir.path / (std::to_string(GetCurrentProcessId()) + ".json");
    REQUIRE(std::filesystem::exists(expected));

    auto parsed = nlohmann::json::parse(read_file(expected));
    CHECK(parsed["protocolVersion"] == 2);
    CHECK(parsed["port"] == 8788);
    CHECK(parsed["token"] == "s3cr3t");
    CHECK(parsed["name"] == "Solo World");
    CHECK(parsed["bind"] == "127.0.0.1");
    CHECK(parsed["pid"] == static_cast<int>(GetCurrentProcessId()));
    CHECK(parsed["startedAt"].get<std::string>().size() == 20);
}

TEST_CASE("write_endpoint_file creates the directory and leaves no temp file") {
    auto root = unique_temp_path();
    TempDirGuard guard(root);
    auto nested = root / "endpoints";
    std::string error;
    REQUIRE(amity::write_endpoint_file(nested, 9000, "t", "n", "0.0.0.0", error));

    int count = 0;
    for (const auto& entry : std::filesystem::directory_iterator(nested)) {
        CHECK(entry.path().extension() == ".json");
        ++count;
    }
    CHECK(count == 1);
}

TEST_CASE("write_endpoint_file overwrites this process's own stale file") {
    TempDirGuard dir(unique_temp_path());
    std::string error;
    REQUIRE(amity::write_endpoint_file(dir.path, 1111, "old", "Old", "127.0.0.1", error));
    REQUIRE(amity::write_endpoint_file(dir.path, 2222, "new", "New", "127.0.0.1", error));

    auto parsed = nlohmann::json::parse(
        read_file(dir.path / (std::to_string(GetCurrentProcessId()) + ".json")));
    CHECK(parsed["port"] == 2222);
    CHECK(parsed["name"] == "New");
}

TEST_CASE("remove_endpoint_file removes only this process's file") {
    TempDirGuard dir(unique_temp_path());
    std::string error;
    REQUIRE(amity::write_endpoint_file(dir.path, 8788, "s3cr3t", "Solo", "127.0.0.1", error));
    auto other = dir.path / "999999.json";
    std::ofstream(other) << "{}";

    amity::remove_endpoint_file(dir.path);
    CHECK_FALSE(std::filesystem::exists(dir.path / (std::to_string(GetCurrentProcessId()) + ".json")));
    CHECK(std::filesystem::exists(other));
}

TEST_CASE("write_endpoint_file reports an empty directory as an error") {
    std::string error;
    CHECK_FALSE(amity::write_endpoint_file({}, 1, "t", "n", "127.0.0.1", error));
    CHECK_FALSE(error.empty());
}

TEST_CASE("default_endpoint_dir ends in the endpoints subdirectory") {
    auto dir = amity::default_endpoint_dir();
    REQUIRE_FALSE(dir.empty());
    CHECK(dir.filename() == "endpoints");
    CHECK(dir.parent_path().filename() == "PSAmity");
}
