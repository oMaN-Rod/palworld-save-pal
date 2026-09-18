#include <amity/mod_paths.hpp>

#include <windows.h>

#include <cstdlib>
#include <fstream>
#include <iterator>
#include <sstream>

namespace amity {

std::filesystem::path config_path_beside_module(const void* address_in_module) {
    HMODULE module = nullptr;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                                GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                            reinterpret_cast<LPCWSTR>(address_in_module), &module)) {
        return {};
    }

    wchar_t buf[4096];
    DWORD len = GetModuleFileNameW(module, buf, static_cast<DWORD>(std::size(buf)));
    if (len == 0 || len >= std::size(buf)) {
        return {};
    }

    std::filesystem::path dll(std::wstring(buf, len));
    return dll.parent_path().parent_path() / "PSAmity.ini";
}

std::filesystem::path game_executable_path() {
    wchar_t buf[4096];
    DWORD len = GetModuleFileNameW(nullptr, buf, static_cast<DWORD>(std::size(buf)));
    if (len == 0 || len >= std::size(buf)) {
        return {};
    }
    return std::filesystem::path(std::wstring(buf, len));
}

BridgeConfig load_config(const std::filesystem::path& ini_path) {
    BridgeConfig cfg;

    if (!ini_path.empty()) {
        std::ifstream in(ini_path, std::ios::binary);
        if (in) {
            std::ostringstream ss;
            ss << in.rdbuf();
            cfg = parse_ini(ss.str());
        }
    }

    apply_env_overrides(cfg, [](const char* key) -> std::optional<std::string> {
        size_t size = 0;
        if (getenv_s(&size, nullptr, 0, key) != 0 || size == 0) {
            return std::nullopt;
        }
        std::string value(size, '\0');
        if (getenv_s(&size, value.data(), size, key) != 0) {
            return std::nullopt;
        }
        value.resize(size == 0 ? 0 : size - 1);
        return value;
    });

    return cfg;
}

}
