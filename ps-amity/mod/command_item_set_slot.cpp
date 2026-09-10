#include "game_commands.hpp"

#include "command_common.hpp"
#include "game_call.hpp"
#include "reflect.hpp"
#include "snapshots.hpp"

#include <amity/item_slot_plan.hpp>

#include <algorithm>
#include <cstddef>
#include <cstring>
#include <string>
#include <utility>
#include <vector>

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt;
using namespace amity_rt::snap;

namespace
{
constexpr int64_t kMinSlotIndex = 0;
constexpr int64_t kMaxSlotIndex = 65535;
constexpr int64_t kMaxCount = 9999;
constexpr size_t kMaxItemIdLength = 128;

constexpr const wchar_t* kItemResultEnum = STR("/Script/Pal.EPalItemOperationResult");

struct AddItemParams
{
    FName StaticItemId{};
    int32_t Count{};
    bool IsAssignPassive{};
    float LogDelay{};
    bool bNotifyLog{};
    uint8_t ReturnValue{};
};

static_assert(sizeof(FName) == 8, "AddItemParams assumes an 8-byte FName, as NameProperty(8) reports");
static_assert(offsetof(AddItemParams, Count) == 8);
static_assert(offsetof(AddItemParams, IsAssignPassive) == 12);
static_assert(offsetof(AddItemParams, LogDelay) == 16);
static_assert(offsetof(AddItemParams, bNotifyLog) == 20);
static_assert(offsetof(AddItemParams, ReturnValue) == 21);

struct ExpectedOffset
{
    const wchar_t* name;
    int32_t offset;
};

struct alignas(8) SlotIdAndNumBlob
{
    uint8_t bytes[24]{};
};

struct FScriptArrayHeader
{
    void* data{};
    int32_t num{};
    int32_t max{};
};

static_assert(sizeof(FScriptArrayHeader) == 16, "a script array is a pointer and two int32 counts");
static_assert(sizeof(TArray<SlotIdAndNumBlob>) == sizeof(FScriptArrayHeader));

void destroy_entries(FStructProperty* element, TArray<SlotIdAndNumBlob>& entries)
{
    for (int32_t i = 0; i < entries.Num(); ++i)
    {
        element->DestroyValue(&entries[i]);
    }
}

nlohmann::json slot_json(const amity::SlotContents& contents)
{
    nlohmann::json entry = nlohmann::json::object();
    entry["staticItemId"] = contents.static_item_id.empty() ? nlohmann::json(nullptr) : nlohmann::json(contents.static_item_id);
    entry["count"] = contents.static_item_id.empty() ? 0 : contents.count;
    return entry;
}

bool read_inventory_containers(const std::string& player_uid_text, nlohmann::json& containers, std::string& error)
{
    amity::GameResponse response = amity_rt::snapshot_inventory(nlohmann::json{{"playerUid", player_uid_text}});
    if (!response.ok())
    {
        error = response.message.empty() ? std::string("inventory read failed") : response.message;
        return false;
    }
    auto found = response.data.find("containers");
    if (found == response.data.end() || !found->is_array())
    {
        error = "inventory read returned no containers";
        return false;
    }
    containers = *found;
    return true;
}

bool read_guild_containers(UObject* player_state, nlohmann::json& containers)
{
    containers = nlohmann::json::array();
    const std::optional<FGuid> guild_id = guild_id_of(guild_of(player_state));
    if (!guild_id)
    {
        return false;
    }
    amity::GameResponse response =
        amity_rt::snapshot_guild_containers(nlohmann::json{{"guildId", format_guid(*guild_id)}});
    auto found = response.ok() ? response.data.find("containers") : response.data.end();
    if (found == response.data.end() || !found->is_array())
    {
        return false;
    }
    containers = *found;
    return true;
}

bool dispose_from_slot(UObject* item_component, const FGuid& container_id, int32_t slot_index, int64_t count)
{
    UFunction* dispose_fn = amity_rt::find_function(STR("/Script/Pal.PalNetworkItemComponent:RequestDispose_ToServer"));
    if (!item_component || !dispose_fn || count <= 0)
    {
        return false;
    }

    ParamBlock block(dispose_fn);
    if (!block.valid())
    {
        return false;
    }
    FStructProperty* slot_info = struct_param(block, {STR("SlotInfo"), STR("Slot")});
    if (!slot_info ||
        !write_slot_id_and_num(slot_info->GetStruct(), block.value_of(slot_info), container_id, slot_index, count))
    {
        return false;
    }
    if (FStructProperty* request_id = struct_param(block, {STR("RequestID"), STR("RequestId")}))
    {
        fill_request_id(request_id->GetStruct(), block.value_of(request_id));
    }
    block.invoke(item_component);
    return true;
}

bool move_into_slot(UObject* item_component,
                     const FGuid& container_id,
                     int32_t slot_index,
                     const std::vector<amity::SlotGain>& sources)
{
    UFunction* move_fn = amity_rt::find_function(STR("/Script/Pal.PalNetworkItemComponent:RequestMove_ToServer"));
    if (!item_component || !move_fn || sources.empty())
    {
        return false;
    }

    ParamBlock block(move_fn);
    if (!block.valid())
    {
        return false;
    }

    FStructProperty* to = struct_param(block, {STR("To"), STR("ToSlot")});
    if (!to || !write_slot_id(to->GetStruct(), block.value_of(to), container_id, slot_index))
    {
        return false;
    }

    auto* froms = CastField<FArrayProperty>(block.param({STR("Froms"), STR("From")}));
    auto* element = froms ? CastField<FStructProperty>(froms->GetInner()) : nullptr;
    UStruct* element_type = element ? element->GetStruct() : nullptr;
    if (!element_type || element->GetSize() != static_cast<int32_t>(sizeof(SlotIdAndNumBlob)) ||
        (froms->GetArrayFlags() & EArrayPropertyFlags::UsesMemoryImageAllocator) != EArrayPropertyFlags::None)
    {
        return false;
    }

    TArray<SlotIdAndNumBlob> entries{};
    for (const amity::SlotGain& source : sources)
    {
        std::optional<FGuid> source_container = parse_guid(source.container_id);
        if (!source_container)
        {
            return false;
        }
        entries.Add(SlotIdAndNumBlob{});
        void* slot = &entries[entries.Num() - 1];
        element->InitializeValue(slot);
        if (!write_slot_id_and_num(element_type, slot, *source_container, source.slot_index, source.gained))
        {
            destroy_entries(element, entries);
            return false;
        }
    }

    if (FStructProperty* request_id = struct_param(block, {STR("RequestID"), STR("RequestId")}))
    {
        fill_request_id(request_id->GetStruct(), block.value_of(request_id));
    }

    // `entries` owns this memory for the whole call. ProcessEvent bitwise-copies the parameter
    // block into its own frame and destroys only what lies past ParmsSize, so the callee never
    // frees it; the array header is cleared afterwards so the block's own teardown does not
    // free it a second time.
    void* froms_value = block.value_of(froms);
    std::memcpy(froms_value, &entries, sizeof(FScriptArrayHeader));
    block.invoke(item_component);
    std::memset(froms_value, 0, sizeof(FScriptArrayHeader));

    destroy_entries(element, entries);
    return true;
}
}

namespace amity_rt
{
bool add_item_layout_matches(UFunction* fn)
{
    static const ExpectedOffset kExpected[] = {
        {STR("StaticItemId"), static_cast<int32_t>(offsetof(AddItemParams, StaticItemId))},
        {STR("Count"), static_cast<int32_t>(offsetof(AddItemParams, Count))},
        {STR("IsAssignPassive"), static_cast<int32_t>(offsetof(AddItemParams, IsAssignPassive))},
        {STR("LogDelay"), static_cast<int32_t>(offsetof(AddItemParams, LogDelay))},
        {STR("bNotifyLog"), static_cast<int32_t>(offsetof(AddItemParams, bNotifyLog))},
        {STR("ReturnValue"), static_cast<int32_t>(offsetof(AddItemParams, ReturnValue))},
    };
    for (const ExpectedOffset& expected : kExpected)
    {
        FProperty* prop = find_struct_prop(fn, {expected.name});
        if (!prop || prop->GetOffset_ForInternal() != expected.offset)
        {
            return false;
        }
    }
    return true;
}

amity::GameResponse command_item_set_slot(const std::string& command_id, const nlohmann::json& args)
{
    FGuid player_uid{};
    std::string error{};
    if (!parse_player_uid(args, player_uid, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }
    const std::string player_uid_text = format_guid(player_uid);

    if (!args.contains("containerId") || !args["containerId"].is_string())
    {
        return amity::GameResponse::fail("validation_failed", "containerId is required");
    }
    const std::string container_id_arg = args["containerId"].get<std::string>();
    std::optional<FGuid> container_id = parse_guid(container_id_arg);
    if (!container_id)
    {
        return amity::GameResponse::fail("validation_failed", "containerId is not a valid GUID");
    }

    if (!args.contains("slotIndex") || !args["slotIndex"].is_number_integer())
    {
        return amity::GameResponse::fail("validation_failed", "slotIndex is required and must be an integer");
    }
    const int64_t slot_index = args["slotIndex"].get<int64_t>();
    if (slot_index < kMinSlotIndex || slot_index > kMaxSlotIndex)
    {
        return amity::GameResponse::fail("validation_failed", "slotIndex is out of range");
    }

    std::string static_item_id{};
    if (args.contains("staticItemId") && !args["staticItemId"].is_null())
    {
        if (!args["staticItemId"].is_string())
        {
            return amity::GameResponse::fail("validation_failed", "staticItemId must be a string");
        }
        static_item_id = args["staticItemId"].get<std::string>();
        if (static_item_id.size() > kMaxItemIdLength)
        {
            return amity::GameResponse::fail("validation_failed", "staticItemId must be 1-128 characters");
        }
    }

    int64_t count = 0;
    if (args.contains("count") && !args["count"].is_null())
    {
        if (!args["count"].is_number_integer())
        {
            return amity::GameResponse::fail("validation_failed", "count must be an integer");
        }
        count = args["count"].get<int64_t>();
        if (count < 0 || count > kMaxCount)
        {
            return amity::GameResponse::fail("validation_failed", "count must be between 0 and 9999");
        }
    }
    else if (!static_item_id.empty())
    {
        return amity::GameResponse::fail("validation_failed", "count is required when staticItemId is set");
    }

    LivePlayer live{};
    if (auto failure = require_live_player(player_uid, live))
    {
        return *failure;
    }
    UObject* player_state = live.player_state;

    UFunction* get_inventory_fn = find_function(STR("/Script/Pal.PalPlayerState:GetInventoryData"));
    UFunction* add_item_fn = find_function(STR("/Script/Pal.PalPlayerInventoryData:AddItem_ServerInternal"));
    if (!get_inventory_fn || !add_item_fn)
    {
        return amity::GameResponse::fail("capability_unavailable", "item slot functions unresolved");
    }

    const char* pawn_reason = "";
    UObject* pawn = player_pawn(player_state, pawn_reason);
    if (!pawn)
    {
        return amity::GameResponse::fail("capability_unavailable", pawn_reason);
    }

    UObject* item_component = item_network_component(pawn);
    if (!item_component)
    {
        return amity::GameResponse::fail("capability_unavailable", "item network component unavailable");
    }

    std::string read_error{};
    nlohmann::json before{};
    if (!read_inventory_containers(player_uid_text, before, read_error))
    {
        return amity::GameResponse::fail("game_error", read_error);
    }

    amity::SlotContents current = amity::read_slot(before, container_id_arg, static_cast<int32_t>(slot_index));
    bool target_in_guild = false;
    if (!current.container_found)
    {
        nlohmann::json guild_containers{};
        if (read_guild_containers(player_state, guild_containers))
        {
            for (const auto& entry : guild_containers)
            {
                before.push_back(entry);
            }
            current = amity::read_slot(before, container_id_arg, static_cast<int32_t>(slot_index));
            target_in_guild = current.container_found;
        }
    }
    if (!current.container_found)
    {
        return amity::GameResponse::fail("validation_failed",
                                         "unknown containerId for this player or their guild");
    }
    if (current.degraded)
    {
        return amity::GameResponse::fail("game_error", "target slot could not be read");
    }

    const amity::SetSlotPlan plan = amity::plan_set_slot(current, static_item_id, count);

    nlohmann::json steps = nlohmann::json::object();
    steps["dispose"] = kStepSkipped;
    steps["grant"] = kStepSkipped;
    steps["move"] = kStepSkipped;

    if (plan.no_op())
    {
        nlohmann::json data = nlohmann::json::object();
        data["steps"] = std::move(steps);
        data["slotBefore"] = slot_json(current);
        data["slotAfter"] = slot_json(current);
        return command_result(command_id, kOpItemSetSlot, false, true, true, std::move(data));
    }

    if (plan.dispose_count > 0)
    {
        const bool disposed = dispose_from_slot(item_component, *container_id, static_cast<int32_t>(slot_index), plan.dispose_count);
        steps["dispose"] = disposed ? kStepOk : kStepFailed;
        if (!disposed)
        {
            return amity::GameResponse::fail("game_error", "could not clear the target slot");
        }
    }

    if (plan.grant_count > 0)
    {
        ObjectReturn inventory_params{};
        player_state->ProcessEvent(get_inventory_fn, &inventory_params);
        if (!inventory_params.ReturnValue)
        {
            return amity::GameResponse::fail("game_error", "inventory data unavailable");
        }

        // FNAME_Find: an id the game never interned cannot name a real item, and the lookup must
        // not add client-supplied strings to the global name table.
        const FName item_name(widen(static_item_id).c_str(), FNAME_Find);
        if (item_name.IsNone())
        {
            return amity::GameResponse::fail("validation_failed", "unknown staticItemId");
        }

        nlohmann::json granted_from{};
        if (!read_inventory_containers(player_uid_text, granted_from, read_error))
        {
            return amity::GameResponse::fail("game_error", read_error);
        }

        AddItemParams add_params{};
        add_params.StaticItemId = item_name;
        add_params.Count = static_cast<int32_t>(plan.grant_count);
        add_params.IsAssignPassive = false;
        add_params.LogDelay = 0.0f;
        add_params.bNotifyLog = true;
        inventory_params.ReturnValue->ProcessEvent(add_item_fn, &add_params);

        std::optional<std::wstring> result_name = enum_name_by_value(kItemResultEnum, static_cast<int64_t>(add_params.ReturnValue));
        const std::string result = result_name ? to_utf8(*result_name) : std::string();
        if (result == "FailedNotFoundStaticItemData")
        {
            steps["grant"] = kStepFailed;
            return amity::GameResponse::fail("validation_failed", "unknown staticItemId");
        }
        if (result != "Success" && result != "SuccessNoOperation")
        {
            steps["grant"] = kStepFailed;
            return amity::GameResponse::fail("game_error", "item grant failed: " + (result.empty() ? std::string("unknown result") : result));
        }
        steps["grant"] = kStepOk;

        nlohmann::json granted_to{};
        if (!read_inventory_containers(player_uid_text, granted_to, read_error))
        {
            return amity::GameResponse::fail("game_error", read_error);
        }

        std::vector<amity::SlotGain> sources = amity::gained_slots(granted_from, granted_to, static_item_id);
        const std::string target_container = format_guid(*container_id);
        sources.erase(std::remove_if(sources.begin(),
                                      sources.end(),
                                      [&](const amity::SlotGain& gain) {
                                          return gain.slot_index == static_cast<int32_t>(slot_index) &&
                                                 gain.container_id == target_container;
                                      }),
                       sources.end());

        if (sources.empty())
        {
            steps["move"] = kStepSkipped;
        }
        else
        {
            const bool moved = move_into_slot(item_component, *container_id, static_cast<int32_t>(slot_index), sources);
            steps["move"] = moved ? kStepOk : kStepFailed;
        }
    }

    nlohmann::json after{};
    if (!read_inventory_containers(player_uid_text, after, read_error))
    {
        return amity::GameResponse::fail("game_error", read_error);
    }
    if (target_in_guild)
    {
        nlohmann::json guild_containers{};
        if (read_guild_containers(player_state, guild_containers))
        {
            for (const auto& entry : guild_containers)
            {
                after.push_back(entry);
            }
        }
    }
    const amity::SlotContents observed = amity::read_slot(after, container_id_arg, static_cast<int32_t>(slot_index));

    const bool wants_empty = static_item_id.empty() || count == 0;
    const bool verified = wants_empty ? observed.static_item_id.empty()
                                      : (observed.static_item_id == static_item_id && observed.count == count);

    nlohmann::json data = nlohmann::json::object();
    data["steps"] = std::move(steps);
    data["slotBefore"] = slot_json(current);
    data["slotAfter"] = slot_json(observed);

    return command_result(command_id, kOpItemSetSlot, true, verified, false, std::move(data));
}
}
