
#include "reflect.hpp"
#include "snapshots.hpp"

#include <string>

#include <Unreal/FField.hpp>

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt;
using namespace amity_rt::snap;

namespace
{
constexpr int kMaxEntries = 400;

nlohmann::json describe(FProperty* prop)
{
    nlohmann::json entry = nlohmann::json::object();
    entry["name"] = to_utf8(prop->GetName());
    entry["type"] = to_utf8(prop->GetClass().GetName());
    const std::wstring declared = declared_type_name(prop);
    entry["declared"] = declared.empty() ? nlohmann::json(nullptr) : nlohmann::json(to_utf8(declared));
    entry["size"] = prop->GetSize();
    entry["offset"] = prop->GetOffset_ForInternal();
    return entry;
}

nlohmann::json properties_of(UStruct* type, bool include_super)
{
    nlohmann::json out = nlohmann::json::array();
    const EFieldIterationFlags flags = include_super ? EFieldIterationFlags::IncludeSuper : EFieldIterationFlags::None;
    for (FProperty* prop : TFieldRange<FProperty>(type, flags))
    {
        if (!prop || out.size() >= kMaxEntries)
        {
            continue;
        }
        out.push_back(describe(prop));
    }
    return out;
}

nlohmann::json element_structs_of(UStruct* type, bool include_super)
{
    nlohmann::json out = nlohmann::json::object();
    const EFieldIterationFlags flags = include_super ? EFieldIterationFlags::IncludeSuper : EFieldIterationFlags::None;
    for (FProperty* prop : TFieldRange<FProperty>(type, flags))
    {
        if (!prop)
        {
            continue;
        }
        FProperty* elements[2] = {nullptr, nullptr};
        if (auto* array_prop = CastField<FArrayProperty>(prop))
        {
            elements[0] = array_prop->GetInner();
        }
        else if (auto* map_prop = CastField<FMapProperty>(prop))
        {
            elements[0] = map_prop->GetKeyProp();
            elements[1] = map_prop->GetValueProp();
        }
        for (FProperty* element : elements)
        {
            auto* struct_element = CastField<FStructProperty>(element);
            UStruct* element_struct = struct_element ? struct_element->GetStruct() : nullptr;
            if (!element_struct)
            {
                continue;
            }
            out[to_utf8(prop->GetName())] = properties_of(element_struct, false);
        }
    }
    return out;
}

nlohmann::json functions_of(UClass* klass)
{
    nlohmann::json out = nlohmann::json::array();
    for (UFunction* fn : TFieldRange<UFunction>(klass, EFieldIterationFlags::None))
    {
        if (!fn || out.size() >= kMaxEntries)
        {
            continue;
        }
        nlohmann::json entry = nlohmann::json::object();
        entry["name"] = to_utf8(fn->GetName());
        nlohmann::json params = nlohmann::json::array();
        for (FProperty* prop : TFieldRange<FProperty>(fn, EFieldIterationFlags::None))
        {
            if (prop)
            {
                params.push_back(describe(prop));
            }
        }
        entry["params"] = std::move(params);
        out.push_back(std::move(entry));
    }
    return out;
}
}

namespace amity_rt
{
amity::GameResponse snapshot_reflect(const nlohmann::json& args)
{
    if (args.contains("search") && args["search"].is_string())
    {
        const std::wstring needle = widen(args["search"].get<std::string>());
        nlohmann::json matches = nlohmann::json::array();
        UObjectGlobals::ForEachUObject([&](UObject* object, int32_t, int32_t) {
            if (!object || matches.size() >= kMaxEntries)
            {
                return LoopAction::Continue;
            }
            const bool is_type = Cast<UClass>(object) || Cast<UScriptStruct>(object);
            if (!is_type)
            {
                return LoopAction::Continue;
            }
            const std::wstring name = object->GetName();
            if (name.find(needle) == std::wstring::npos)
            {
                return LoopAction::Continue;
            }
            matches.push_back(to_utf8(object->GetFullName()));
            return LoopAction::Continue;
        });

        amity::GameResponse response;
        response.data = {
            {"search", args["search"]},
            {"matches", std::move(matches)},
        };
        return response;
    }

    if (args.contains("holder") && args["holder"].is_string())
    {
        const std::wstring needle = widen(args["holder"].get<std::string>());
        nlohmann::json holders = nlohmann::json::array();
        UObjectGlobals::ForEachUObject([&](UObject* object, int32_t, int32_t) {
            auto* klass = Cast<UClass>(object);
            if (!klass || holders.size() >= kMaxEntries)
            {
                return LoopAction::Continue;
            }
            for (FProperty* prop : TFieldRange<FProperty>(klass, EFieldIterationFlags::None))
            {
                if (!prop)
                {
                    continue;
                }
                const std::wstring declared = declared_type_name(prop);
                if (declared.find(needle) == std::wstring::npos)
                {
                    continue;
                }
                nlohmann::json entry = nlohmann::json::object();
                entry["class"] = to_utf8(klass->GetFullName());
                entry["property"] = to_utf8(prop->GetName());
                entry["declared"] = to_utf8(declared);
                holders.push_back(std::move(entry));
            }
            return LoopAction::Continue;
        });

        amity::GameResponse response;
        response.data = {
            {"holder", args["holder"]},
            {"holders", std::move(holders)},
        };
        return response;
    }

    if (!args.contains("path") || !args["path"].is_string())
    {
        return amity::GameResponse::fail("validation_failed", "path, search or holder is required");
    }
    std::wstring path = widen(args["path"].get<std::string>());
    if (const size_t space = path.find(L' '); space != std::wstring::npos && path.find(L'/') > space)
    {
        path = path.substr(space + 1);
    }
    const bool include_super = !args.contains("includeSuper") || args["includeSuper"].get<bool>();

    nlohmann::json data = nlohmann::json::object();
    data["path"] = args["path"];

    UObject* found = UObjectGlobals::StaticFindObject<UObject*>(nullptr, nullptr, path.c_str());
    if (!found)
    {
        return amity::GameResponse::fail("validation_failed", "nothing at that path");
    }

    UStruct* type = nullptr;
    if (auto* klass = Cast<UClass>(found))
    {
        data["kind"] = "class";
        type = klass;
        data["functions"] = functions_of(klass);
    }
    else if (auto* fn = Cast<UFunction>(found))
    {
        data["kind"] = "function";
        type = fn;
        data["functions"] = nlohmann::json::array();
    }
    else if (auto* strct = Cast<UScriptStruct>(found))
    {
        data["kind"] = "struct";
        type = strct;
        data["functions"] = nlohmann::json::array();
    }
    else
    {
        data["kind"] = to_utf8(found->GetClassPrivate()->GetName());
        data["functions"] = nlohmann::json::array();
    }

    data["name"] = to_utf8(found->GetFullName());
    const bool walk_super = include_super && data["kind"] != "function";
    data["properties"] = type ? properties_of(type, walk_super) : nlohmann::json::array();
    data["elementStructs"] = type ? element_structs_of(type, walk_super) : nlohmann::json::object();

    amity::GameResponse response;
    response.data = std::move(data);
    return response;
}
}
