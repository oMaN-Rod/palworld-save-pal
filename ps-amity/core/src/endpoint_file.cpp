#include <amity/endpoint_file.hpp>
#include <amity/protocol.hpp>

#include <windows.h>

#include <nlohmann/json.hpp>
#include <chrono>
#include <cstdio>
#include <ctime>
#include <fstream>
#include <iterator>
#include <system_error>

namespace amity {

namespace {

std::string current_time_iso8601_utc() {
    std::time_t t = std::chrono::system_clock::to_time_t(std::chrono::system_clock::now());
    std::tm tm{};
    gmtime_s(&tm, &t);
    char buf[32];
    std::snprintf(buf, sizeof(buf), "%04d-%02d-%02dT%02d:%02d:%02dZ", tm.tm_year + 1900, tm.tm_mon + 1,
                  tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec);
    return std::string(buf);
}

}

std::filesystem::path default_endpoint_dir() {
    wchar_t buf[4096];
    DWORD len = GetEnvironmentVariableW(L"LOCALAPPDATA", buf, static_cast<DWORD>(std::size(buf)));
    if (len == 0 || len >= std::size(buf)) {
        return std::filesystem::path();
    }
    return std::filesystem::path(std::wstring(buf, len)) / "Pal" / "Saved" / "PSAmity" / "endpoints";
}

bool write_endpoint_file(const std::filesystem::path& dir, int port, const std::string& token,
                         const std::string& name, const std::string& bind, std::string& error) {
    if (dir.empty()) {
        error = "endpoint directory is empty (LOCALAPPDATA not resolved)";
        return false;
    }

    std::error_code ec;
    std::filesystem::create_directories(dir, ec);
    if (ec) {
        error = "failed to create endpoint directory: " + ec.message();
        return false;
    }

    std::string filename = std::to_string(static_cast<int>(GetCurrentProcessId()));

    nlohmann::json j = {
        {"protocolVersion", PROTOCOL_VERSION},
        {"port", port},
        {"token", token},
        {"name", name},
        {"bind", bind},
        {"pid", static_cast<int>(GetCurrentProcessId())},
        {"startedAt", current_time_iso8601_utc()},
    };
    std::string content = j.dump();

    std::filesystem::path target = dir / (filename + ".json");
    std::filesystem::path temp = dir / (filename + ".json.tmp");

    {
        std::ofstream out(temp, std::ios::binary | std::ios::trunc);
        if (!out) {
            error = "failed to open temp endpoint file for writing";
            return false;
        }
        out.write(content.data(), static_cast<std::streamsize>(content.size()));
        if (!out) {
            error = "failed to write endpoint file content";
            out.close();
            std::filesystem::remove(temp, ec);
            return false;
        }
    }

    std::filesystem::rename(temp, target, ec);
    if (ec) {
        error = "failed to atomically replace endpoint file: " + ec.message();
        std::filesystem::remove(temp, ec);
        return false;
    }

    return true;
}

void remove_endpoint_file(const std::filesystem::path& dir) {
    std::error_code ec;
    std::filesystem::remove(dir / (std::to_string(static_cast<int>(GetCurrentProcessId())) + ".json"), ec);
}

}
