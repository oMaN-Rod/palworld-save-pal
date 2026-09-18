#include "snapshots.hpp"

#include "amity_mod.hpp"

#include <amity/mod_paths.hpp>

#include <Mod/CppMod.hpp>
#include <Mod/LuaMod.hpp>
#include <UE4SSProgram.hpp>
#include <Unreal/UnrealVersion.hpp>

#include <algorithm>
#include <cwctype>
#include <map>
#include <string>

using namespace amity_rt::snap;

namespace
{
const int kModuleAnchor = 0;

bool has_component(const std::filesystem::path& path, std::wstring_view wanted)
{
    for (const auto& part : path)
    {
        std::wstring text = part.wstring();
        std::transform(text.begin(), text.end(), text.begin(), [](wchar_t c) { return static_cast<wchar_t>(std::towlower(c)); });
        if (text == wanted)
        {
            return true;
        }
    }
    return false;
}
} // namespace

namespace amity_rt
{
amity::GameResponse snapshot_build_info()
{
    const std::filesystem::path config = amity::config_path_beside_module(&kModuleAnchor);
    const std::filesystem::path game_executable = amity::game_executable_path();
    amity::GameResponse response;
    response.data = {
        {"gameVersion", nullptr},
        {"engineVersion", std::to_string(RC::Unreal::Version::Major) + "." + std::to_string(RC::Unreal::Version::Minor)},
        {"ue4ssVersion", std::to_string(UE4SS_LIB_VERSION_MAJOR) + "." + std::to_string(UE4SS_LIB_VERSION_MINOR) + "." + std::to_string(UE4SS_LIB_VERSION_HOTFIX)},
        {"ue4ssBuild", UE4SS_LIB_BUILD_GITSHA},
        {"amityVersion", AMITY_VERSION},
        {"platform", has_component(game_executable, L"wingdk") ? "wingdk" : "win64"},
        {"ue4ssMode", has_component(config, L"nativemods") ? "workshop" : "standard"},
    };
    return response;
}

amity::GameResponse snapshot_loaded_mods()
{
    struct Loaded
    {
        bool enabled = false;
        bool has_lua = false;
        bool has_dll = false;
    };
    std::map<std::string, Loaded> by_name;
    for (const auto& mod : RC::UE4SSProgram::get_program().m_mods)
    {
        Loaded& loaded = by_name[to_utf8(std::wstring(mod->get_name()))];
        loaded.enabled = loaded.enabled || mod->is_started();
        loaded.has_lua = loaded.has_lua || dynamic_cast<RC::LuaMod*>(mod.get()) != nullptr;
        loaded.has_dll = loaded.has_dll || dynamic_cast<RC::CppMod*>(mod.get()) != nullptr;
    }
    nlohmann::json list = nlohmann::json::array();
    for (const auto& [name, loaded] : by_name)
    {
        list.push_back({{"name", name}, {"enabled", loaded.enabled}, {"hasLua", loaded.has_lua}, {"hasDll", loaded.has_dll}});
    }
    amity::GameResponse response;
    response.data = {{"ue4ss", list}};
    return response;
}
} // namespace amity_rt
