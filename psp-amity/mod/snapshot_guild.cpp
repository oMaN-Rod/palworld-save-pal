#include "command_common.hpp"
#include "reflect.hpp"
#include "snapshots.hpp"

#include <cstdint>
#include <string>
#include <vector>

#include <Unreal/FField.hpp>

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt;
using namespace amity_rt::snap;

namespace
{
// Every element of a `TArray<FGuid>` on `owner`, or null if the property is not an array of
// exactly guid-sized structs. FArrayProperty::GetSize() is 16 for every TArray whatever it
// holds, so the stride of the inner is the only thing that says these are guids.
nlohmann::json guid_array(UObject* owner, const wchar_t* name)
{
    auto* array_prop = CastField<FArrayProperty>(find_prop(owner, {name}));
    if (!array_prop)
    {
        return nullptr;
    }
    auto* inner = CastField<FStructProperty>(array_prop->GetInner());
    if (!inner || inner->GetSize() != static_cast<int32_t>(sizeof(FGuid)))
    {
        return nullptr;
    }

    nlohmann::json out = nlohmann::json::array();
    FScriptArrayHelper helper(array_prop, array_prop->ContainerPtrToValuePtr<void>(owner));
    for (int32_t i = 0; i < helper.Num(); ++i)
    {
        if (uint8_t* element = helper.GetRawPtr(i))
        {
            out.push_back(format_guid(*reinterpret_cast<FGuid*>(element)));
        }
    }
    return out;
}

nlohmann::json string_member(UObject* owner, const wchar_t* name)
{
    if (FString* text = value_ptr<FString>(find_prop(owner, {name}), owner))
    {
        return to_utf8(text->operator*());
    }
    return nullptr;
}

nlohmann::json struct_enum_name(UStruct* type, void* base, const wchar_t* name)
{
    auto* enum_prop = CastField<FEnumProperty>(find_struct_prop(type, {name}));
    FNumericProperty* underlying = enum_prop ? enum_prop->GetUnderlyingProperty() : nullptr;
    UEnum* enum_type = enum_prop ? enum_prop->GetEnum() : nullptr;
    if (!underlying || !enum_type)
    {
        return nullptr;
    }
    const int64_t value = underlying->GetSignedIntPropertyValue(enum_prop->ContainerPtrToValuePtr<void>(base));
    return bare_enumerator(to_utf8(enum_type->GetNameByValue(value).ToString()));
}

nlohmann::json read_members(UObject* guild)
{
    auto* fast_struct = CastField<FStructProperty>(find_prop(guild, {STR("PlayerInfoRepInfoArray")}));
    if (fast_struct && fast_struct->GetStruct())
    {
        void* fast_ptr = fast_struct->ContainerPtrToValuePtr<void>(guild);
        auto* items = CastField<FArrayProperty>(find_struct_prop(fast_struct->GetStruct(), {STR("Items")}));
        auto* row = items ? CastField<FStructProperty>(items->GetInner()) : nullptr;
        UStruct* row_type = row ? row->GetStruct() : nullptr;
        if (!items || !row_type)
        {
            return nullptr;
        }

        nlohmann::json out = nlohmann::json::array();
        FScriptArrayHelper helper(items, items->ContainerPtrToValuePtr<void>(fast_ptr));
        for (int32_t i = 0; i < helper.Num(); ++i)
        {
            uint8_t* element = helper.GetRawPtr(i);
            if (!element)
            {
                continue;
            }
            nlohmann::json entry = nlohmann::json::object();
            entry["uid"] = nullptr;
            if (FGuid* uid = value_ptr<FGuid>(find_struct_prop(row_type, {STR("PlayerUId")}), element))
            {
                entry["uid"] = format_guid(*uid);
            }

            entry["name"] = nullptr;
            entry["role"] = nullptr;
            entry["status"] = nullptr;
            entry["lastOnlineTicks"] = nullptr;
            auto* info_prop = CastField<FStructProperty>(find_struct_prop(row_type, {STR("PlayerInfo")}));
            UStruct* info_type = info_prop ? info_prop->GetStruct() : nullptr;
            if (info_type)
            {
                void* info = info_prop->ContainerPtrToValuePtr<void>(element);
                if (FString* name = value_ptr<FString>(find_struct_prop(info_type, {STR("PlayerName")}), info))
                {
                    entry["name"] = to_utf8(name->operator*());
                }
                entry["role"] = struct_enum_name(info_type, info, STR("Role"));
                entry["status"] = struct_enum_name(info_type, info, STR("Status"));
                if (int64_t* ticks = value_ptr<int64_t>(find_struct_prop(info_type, {STR("LastOnlineRealTime")}), info))
                {
                    entry["lastOnlineTicks"] = *ticks;
                }
            }
            out.push_back(std::move(entry));
        }
        return out;
    }
    return nullptr;
}

nlohmann::json read_lab(UObject* guild)
{
    UObject* lab = snap::object_member(guild, STR("Lab"));
    if (!lab)
    {
        return nullptr;
    }

    nlohmann::json entry = nlohmann::json::object();
    entry["currentResearchId"] = nullptr;
    if (FName* current = value_ptr<FName>(find_prop(lab, {STR("CurrentResearchId")}), lab))
    {
        entry["currentResearchId"] = to_utf8(current->ToString());
    }

    entry["research"] = nlohmann::json::array();
    auto* fast_struct = CastField<FStructProperty>(find_prop(lab, {STR("ResearchRepInfoArray")}));
    if (fast_struct && fast_struct->GetStruct())
    {
        void* fast_ptr = fast_struct->ContainerPtrToValuePtr<void>(lab);
        auto* items = CastField<FArrayProperty>(find_struct_prop(fast_struct->GetStruct(), {STR("Items")}));
        auto* row = items ? CastField<FStructProperty>(items->GetInner()) : nullptr;
        UStruct* row_type = row ? row->GetStruct() : nullptr;
        if (items && row_type)
        {
            FScriptArrayHelper helper(items, items->ContainerPtrToValuePtr<void>(fast_ptr));
            for (int32_t i = 0; i < helper.Num(); ++i)
            {
                uint8_t* element = helper.GetRawPtr(i);
                if (!element)
                {
                    continue;
                }
                nlohmann::json research = nlohmann::json::object();
                research["researchId"] = nullptr;
                if (FName* id = value_ptr<FName>(find_struct_prop(row_type, {STR("ResearchId")}), element))
                {
                    research["researchId"] = to_utf8(id->ToString());
                }
                research["workAmount"] = nullptr;
                if (float* work = value_ptr<float>(find_struct_prop(row_type, {STR("WorkAmount")}), element))
                {
                    research["workAmount"] = *work;
                }
                research["requiredWorkAmount"] = nullptr;
                if (float* required = value_ptr<float>(find_struct_prop(row_type, {STR("RequiredWorkAmount")}), element))
                {
                    research["requiredWorkAmount"] = *required;
                }
                entry["research"].push_back(std::move(research));
            }
        }
    }
    return entry;
}

nlohmann::json read_bases(const FGuid& guild_id)
{
    nlohmann::json out = nlohmann::json::array();
    std::vector<UObject*> models{};
    UObjectGlobals::FindAllOf(STR("PalBaseCampModel"), models);
    for (UObject* model : models)
    {
        if (!model)
        {
            continue;
        }
        FGuid* group_id = value_ptr<FGuid>(find_prop(model, {STR("GroupIdBelongTo")}), model);
        if (!group_id || !(*group_id == guild_id))
        {
            continue;
        }

        nlohmann::json entry = nlohmann::json::object();
        FGuid* id = value_ptr<FGuid>(find_prop(model, {STR("ID")}), model);
        entry["id"] = id ? nlohmann::json(format_guid(*id)) : nlohmann::json(nullptr);
        entry["name"] = string_member(model, STR("BaseCampName"));
        if (auto level = read_numeric(find_prop(model, {STR("Level_InGuildProperty")}), model))
        {
            entry["level"] = *level;
        }
        else
        {
            entry["level"] = nullptr;
        }
        if (auto buildings = read_numeric(find_prop(model, {STR("BuildingNum")}), model))
        {
            entry["buildingNum"] = *buildings;
        }

        entry["containerId"] = nullptr;
        entry["palSlotNum"] = nullptr;
        if (UObject* director = object_member(model, STR("WorkerDirector")))
        {
            if (UObject* container = object_member(director, STR("CharacterContainer")))
            {
                entry["containerId"] = nested_guid(container, STR("ID"), STR("ID"));
                if (auto* slots = CastField<FArrayProperty>(find_prop(container, {STR("SlotArray")})))
                {
                    FScriptArrayHelper helper(slots, slots->ContainerPtrToValuePtr<void>(container));
                    entry["palSlotNum"] = helper.Num();
                }
            }
        }
        out.push_back(std::move(entry));
    }
    return out;
}

}

namespace amity_rt
{
// The roster lives on a `PalGuildInfo` actor, which the guild points at through a WEAK
// property. That is deliberately not dereferenced: a weak pointer stores an index and serial
// rather than an address, so reading it as one is the container-vs-value-pointer mistake in a
// new place. The actor carries the same `GroupId` the guild is keyed by, so it is found by
// matching that instead -- which also reaches guilds nobody is standing in.
UObject* guild_info_for(const FGuid& guild_id)
{
    std::vector<UObject*> instances{};
    UObjectGlobals::FindAllOf(STR("PalGuildInfo"), instances);
    for (UObject* instance : instances)
    {
        if (!instance)
        {
            continue;
        }
        FGuid* group_id = value_ptr<FGuid>(find_prop(instance, {STR("GroupId")}), instance);
        if (group_id && *group_id == guild_id)
        {
            return instance;
        }
    }
    return nullptr;
}

UObject* guild_by_id(const FGuid& guild_id)
{
    std::vector<UObject*> guilds{};
    UObjectGlobals::FindAllOf(STR("PalGroupGuildBase"), guilds);
    for (UObject* guild : guilds)
    {
        if (!guild)
        {
            continue;
        }
        FGuid* id = value_ptr<FGuid>(find_prop(guild, {STR("ID")}), guild);
        if (id && *id == guild_id)
        {
            return guild;
        }
    }
    return nullptr;
}

amity::GameResponse snapshot_guild(const nlohmann::json& args)
{
    amity::GameResponse response;
    UObject* guild = nullptr;

    if (args.contains("guildId") && args["guildId"].is_string())
    {
        const std::optional<FGuid> guild_id = parse_guid(args["guildId"].get<std::string>());
        if (!guild_id)
        {
            return amity::GameResponse::fail("validation_failed", "guildId is not a valid GUID");
        }
        guild = guild_by_id(*guild_id);
        if (!guild)
        {
            return amity::GameResponse::fail("validation_failed", "unknown guildId");
        }
    }
    else
    {
        if (!args.contains("playerUid") || !args["playerUid"].is_string())
        {
            return amity::GameResponse::fail("validation_failed", "playerUid or guildId is required");
        }
        const std::optional<FGuid> player_uid = parse_guid(args["playerUid"].get<std::string>());
        if (!player_uid)
        {
            return amity::GameResponse::fail("validation_failed", "playerUid is not a valid GUID");
        }

        LivePlayer live{};
        if (auto failure = require_live_player(*player_uid, live))
        {
            return *failure;
        }
        UObject* player_state = live.player_state;

        auto* guild_prop = CastField<FObjectPropertyBase>(find_prop(player_state, {STR("GuildBelongTo")}));
        if (!guild_prop)
        {
            response.data = {{"guild", nullptr}, {"status", "partial"}};
            return response;
        }
        guild = guild_prop->GetObjectPropertyValue(guild_prop->ContainerPtrToValuePtr<void>(player_state));
        if (!guild)
        {
            response.data = {{"guild", nullptr}, {"status", "ok"}};
            return response;
        }
    }

    bool degraded = false;
    nlohmann::json entry = nlohmann::json::object();

    FGuid* id = value_ptr<FGuid>(find_prop(guild, {STR("ID")}), guild);
    entry["id"] = id ? nlohmann::json(format_guid(*id)) : nlohmann::json(nullptr);
    degraded = degraded || !id;

    entry["name"] = string_member(guild, STR("GuildName"));
    entry["groupName"] = string_member(guild, STR("GroupName"));
    degraded = degraded || entry["name"].is_null();

    if (auto level = read_numeric(find_prop(guild, {STR("BaseCampLevel")}), guild))
    {
        entry["baseCampLevel"] = *level;
    }
    else
    {
        entry["baseCampLevel"] = nullptr;
        degraded = true;
    }

    entry["baseCampIds"] = guid_array(guild, STR("BaseCampIds"));
    entry["baseCampPointIds"] = guid_array(guild, STR("MapObjectInstanceIds_BaseCampPoint"));

    entry["memberUids"] = nullptr;
    if (id)
    {
        if (UObject* info = guild_info_for(*id))
        {
            entry["memberUids"] = guid_array(info, STR("PlayerUIds"));
        }
    }
    degraded = degraded || entry["memberUids"].is_null();

    entry["bases"] = id ? read_bases(*id) : nlohmann::json(nullptr);
    degraded = degraded || entry["bases"].is_null();

    entry["adminUid"] = nullptr;
    if (FGuid* admin = value_ptr<FGuid>(find_prop(guild, {STR("AdminPlayerUId")}), guild))
    {
        entry["adminUid"] = format_guid(*admin);
    }

    entry["members"] = read_members(guild);
    entry["roleOptions"] = nullptr;
    if (auto* row_struct = find_script_struct(STR("/Script/Pal.PalGuildPlayerInfo")))
    {
        auto* role_prop = CastField<FEnumProperty>(find_struct_prop(row_struct, {STR("Role")}));
        entry["roleOptions"] = enum_names(role_prop ? role_prop->GetEnum() : nullptr);
    }
    entry["lab"] = read_lab(guild);

    response.data = {
        {"guild", std::move(entry)},
        {"status", degraded ? "partial" : "ok"},
    };
    return response;
}
}

namespace amity_rt
{
amity::GameResponse snapshot_guilds(const nlohmann::json&)
{
    std::vector<UObject*> guilds{};
    UObjectGlobals::FindAllOf(STR("PalGroupGuildBase"), guilds);

    nlohmann::json out = nlohmann::json::array();
    for (UObject* guild : guilds)
    {
        if (!guild)
        {
            continue;
        }
        nlohmann::json entry = nlohmann::json::object();
        FGuid* id = value_ptr<FGuid>(find_prop(guild, {STR("ID")}), guild);
        entry["id"] = id ? nlohmann::json(format_guid(*id)) : nlohmann::json(nullptr);
        entry["name"] = string_member(guild, STR("GuildName"));
        entry["adminUid"] = nullptr;
        if (FGuid* admin = value_ptr<FGuid>(find_prop(guild, {STR("AdminPlayerUId")}), guild))
        {
            entry["adminUid"] = format_guid(*admin);
        }
        entry["members"] = read_members(guild);
        out.push_back(std::move(entry));
    }

    amity::GameResponse response;
    response.data = {{"guilds", std::move(out)}, {"status", "ok"}};
    return response;
}
}
