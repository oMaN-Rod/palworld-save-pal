#include "game_commands.hpp"

#include "command_common.hpp"
#include "game_call.hpp"
#include "reflect.hpp"
#include "snapshots.hpp"

#include <DynamicOutput/DynamicOutput.hpp>
#include <Unreal/FField.hpp>

#include <cstdint>
#include <memory>
#include <optional>
#include <random>
#include <string>
#include <utility>

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt;
using namespace amity_rt::snap;

namespace
{
constexpr int64_t kMinSlotIndex = 0;
constexpr int64_t kMaxSlotIndex = 959;
constexpr int64_t kMinLevel = 1;
constexpr int64_t kMaxLevel = 255;
constexpr size_t kMaxCharacterIdLength = 128;

struct SlotIdReturn
{
    FGuid ContainerId{};
    int32_t SlotIndex{};
};

static_assert(sizeof(SlotIdReturn) == 20, "PalCharacterSlotId is a 16-byte container id and an int32 index");

bool slot_id_of(UObject* slot, FGuid& container_id, int32_t& slot_index)
{
    UFunction* get_slot_id_fn = find_function(STR("/Script/Pal.PalIndividualCharacterSlot:GetSlotId"));
    if (!slot || !get_slot_id_fn)
    {
        return false;
    }
    SlotIdReturn params{};
    slot->ProcessEvent(get_slot_id_fn, &params);
    container_id = params.ContainerId;
    slot_index = params.SlotIndex;
    return true;
}

bool slot_occupied(UObject* world_context, const FGuid& player_uid, int32_t slot_index)
{
    std::string ignored_code{}, ignored_message{};
    return individual_parameter_by_slot_index(world_context, player_uid, slot_index, ignored_code, ignored_message) !=
            nullptr;
}

std::string instance_id_at(UObject* world_context, const FGuid& player_uid, int32_t slot_index)
{
    std::string ignored_code{}, ignored_message{};
    return instance_id_of_parameter(
        individual_parameter_by_slot_index(world_context, player_uid, slot_index, ignored_code, ignored_message));
}

bool slot_id_of_parameter(UObject* parameter, FGuid& container_id, int32_t& slot_index)
{
    auto* save_prop = parameter ? CastField<FStructProperty>(find_prop(parameter, {STR("SaveParameter")})) : nullptr;
    UStruct* save_type = save_prop ? save_prop->GetStruct() : nullptr;
    if (!save_type)
    {
        return false;
    }
    void* save_value = save_prop->ContainerPtrToValuePtr<void>(parameter);

    auto* slot_prop = CastField<FStructProperty>(find_struct_prop(save_type, {STR("SlotId"), STR("SlotID")}));
    UStruct* slot_type = slot_prop ? slot_prop->GetStruct() : nullptr;
    if (!slot_type)
    {
        return false;
    }
    void* slot_value = slot_prop->ContainerPtrToValuePtr<void>(save_value);

    auto* container_prop = CastField<FStructProperty>(find_struct_prop(slot_type, {STR("ContainerId"), STR("ContainerID")}));
    if (!container_prop)
    {
        return false;
    }
    auto* guid_prop = CastField<FStructProperty>(find_struct_prop(container_prop->GetStruct(), {STR("ID"), STR("Id")}));
    FGuid* guid = guid_prop ? value_ptr<FGuid>(guid_prop, container_prop->ContainerPtrToValuePtr<void>(slot_value))
                             : nullptr;
    std::optional<int64_t> index = read_numeric(find_struct_prop(slot_type, {STR("SlotIndex")}), slot_value);
    if (!guid || !index)
    {
        return false;
    }
    container_id = *guid;
    slot_index = static_cast<int32_t>(*index);
    return true;
}

struct AddOtomoParams
{
    UObject* Handle{};
    bool ReturnValue{};
};

bool give_handle_to_party(UObject* player_state, UObject* handle)
{
    UObject* holder = otomo_holder(player_state);
    UFunction* add_fn = find_function(STR("/Script/Pal.PalOtomoHolderComponentBase:AddOtomoHandleToFreeSlot"));
    if (!holder || !add_fn || !handle)
    {
        return false;
    }
    AddOtomoParams params{};
    params.Handle = handle;
    holder->ProcessEvent(add_fn, &params);
    return params.ReturnValue;
}

bool swap_slots(UObject* container_component,
                 const FGuid& a_container,
                 int32_t a_index,
                 const FGuid& b_container,
                 int32_t b_index)
{
    UFunction* swap_fn =
        find_function(STR("/Script/Pal.PalNetworkCharacterContainerComponent:RequestSwap_ToServer_Rep"));
    if (!container_component || !swap_fn)
    {
        return false;
    }
    ParamBlock block(swap_fn);
    FStructProperty* a_param = block.valid() ? struct_param(block, {STR("SlotIdA"), STR("SlotA")}) : nullptr;
    FStructProperty* b_param = block.valid() ? struct_param(block, {STR("SlotIdB"), STR("SlotB")}) : nullptr;
    if (!a_param || !b_param ||
        !write_slot_id(a_param->GetStruct(), block.value_of(a_param), a_container, a_index) ||
        !write_slot_id(b_param->GetStruct(), block.value_of(b_param), b_container, b_index))
    {
        return false;
    }
    block.invoke(container_component);
    return true;
}

FGuid random_guid()
{
    static std::mt19937_64 engine{std::random_device{}()};
    std::uniform_int_distribution<uint32_t> any{};
    FGuid guid{};
    guid.A = any(engine);
    guid.B = (any(engine) & 0xFFFF0FFFu) | 0x00004000u;
    guid.C = (any(engine) & 0x3FFFFFFFu) | 0x80000000u;
    guid.D = any(engine);
    return guid;
}

struct ParameterObjectParams
{
    UObject* Parameter{};
    UObject* ReturnValue{};
};

UObject* handle_of_parameter(UObject* manager, UObject* parameter)
{
    UFunction* fn = find_function(STR("/Script/Pal.PalCharacterManager:GetIndividualHandleFromCharacterParameter"));
    if (!manager || !parameter || !fn)
    {
        return nullptr;
    }
    ParameterObjectParams params{};
    params.Parameter = parameter;
    manager->ProcessEvent(fn, &params);
    return params.ReturnValue;
}

bool setup_save_parameter(UObject* world_context,
                           const FName& character_id,
                           int64_t level,
                           const FGuid& owner_uid,
                           FStructProperty* destination_prop,
                           void* destination_value)
{
    UFunction* get_database_fn = find_function(STR("/Script/Pal.PalUtility:GetDatabaseCharacterParameter"));
    UFunction* setup_fn = find_function(STR("/Script/Pal.PalDatabaseCharacterParameter:SetupSaveParameter"));
    if (!world_context || !get_database_fn || !setup_fn || !destination_prop || !destination_value)
    {
        return false;
    }

    WorldContextObjectReturnParams database_params{};
    database_params.WorldContextObject = world_context;
    world_context->ProcessEvent(get_database_fn, &database_params);
    UObject* database = database_params.ReturnValue;
    if (!database)
    {
        return false;
    }

    ParamBlock block(setup_fn);
    if (!block.valid())
    {
        return false;
    }
    FStructProperty* out_prop = struct_param(block, {STR("outParameter"), STR("OutParameter")});
    FName* character_value = block.name_at({STR("CharacterID"), STR("CharacterId")});
    FGuid* owner_value = block.guid_at({STR("OwnerPlayerUId")});
    if (!character_value || !owner_value || !out_prop || out_prop->GetSize() != destination_prop->GetSize() ||
        !block.set_integral({STR("Level")}, level))
    {
        return false;
    }
    *character_value = character_id;
    *owner_value = owner_uid;
    block.invoke(database);

    if (!block.bool_at({STR("ReturnValue")}))
    {
        return false;
    }
    destination_prop->CopyCompleteValue(destination_value, block.value_of(out_prop));
    return true;
}

// The game finishes creating a pal on a later tick than the call that asks for it, so every
// stage below runs on its own tick and nothing touches a result in the tick that produced it.
// No game object is held between ticks: the pal is found again each time by the id the job
// chose for it, through the manager.
constexpr int kReadyWaitTicks = 300;
constexpr int kPartySeats = 5;

enum class AddTarget
{
    Palbox,
    Base,
    Party
};

struct PalAddJob
{
    std::string command_id;
    FGuid player_uid{};
    FGuid instance_id{};
    AddTarget target{AddTarget::Palbox};
    FGuid base_id{};
    int32_t slot_index{};
    FGuid target_container{};
    int32_t target_index{};
    int waited{};
    bool borrowed{};
    std::string borrowed_instance{};
    nlohmann::json steps = nlohmann::json::object();
};

std::string instance_id_of_handle(UObject* handle)
{
    auto* id_prop = handle ? CastField<FStructProperty>(find_prop(handle, {STR("ID")})) : nullptr;
    if (!id_prop)
    {
        return {};
    }
    FGuid* instance = value_ptr<FGuid>(find_struct_prop(id_prop->GetStruct(), {STR("InstanceId")}),
                                        id_prop->ContainerPtrToValuePtr<void>(handle));
    return instance ? format_guid(*instance) : std::string();
}

std::string instance_id_at_base(const FGuid& base_id, int32_t slot_index)
{
    const char* ignored = "";
    UObject* slot = base_container_slot(base_camp_container(base_id, ignored), slot_index);
    return instance_id_of_handle(slot ? object_member(slot, STR("Handle")) : nullptr);
}

std::string instance_id_at_target(const std::shared_ptr<PalAddJob>& job, UObject* world_context)
{
    if (job->target == AddTarget::Base)
    {
        return instance_id_at_base(job->base_id, job->slot_index);
    }
    return world_context ? instance_id_at(world_context, job->player_uid, job->slot_index) : std::string();
}

int32_t party_seat_of(UObject* holder, const std::string& instance_id)
{
    for (int32_t seat = 0; seat < kPartySeats; ++seat)
    {
        if (instance_id_of_parameter(party_parameter_at(holder, seat)) == instance_id)
        {
            return seat;
        }
    }
    return -1;
}

bool party_holds(UObject* holder, const std::string& instance_id)
{
    return party_seat_of(holder, instance_id) >= 0;
}

bool party_has_free_seat(UObject* holder)
{
    for (int32_t seat = 0; seat < kPartySeats; ++seat)
    {
        if (!party_parameter_at(holder, seat))
        {
            return true;
        }
    }
    return false;
}

amity::GameResponse pal_add_finish(std::shared_ptr<PalAddJob> job)
{
    UObject* world_context = any_player_controller();
    UObject* player_state = world_context ? player_state_by_uid(world_context, job->player_uid) : nullptr;
    const std::string wanted = format_guid(job->instance_id);

    bool verified = false;
    if (job->target == AddTarget::Party)
    {
        job->slot_index = party_seat_of(otomo_holder(player_state), wanted);
        verified = job->slot_index >= 0;
    }
    else
    {
        const std::string at_target = instance_id_at_target(job, world_context);
        verified = !at_target.empty() && at_target == wanted;
    }

    if (job->borrowed)
    {
        const bool home = party_holds(otomo_holder(player_state), job->borrowed_instance);
        job->steps["restore"] = home ? kStepOk : kStepFailed;
        verified = verified && home;
    }

    Output::send<LogLevel::Verbose>(STR("[PSAmity] pal.add: {} inSlot={} steps={}\n"),
                                     widen(wanted),
                                     verified,
                                     widen(job->steps.dump()));

    nlohmann::json data = nlohmann::json::object();
    data["steps"] = job->steps;
    data["created"] = verified ? nlohmann::json(wanted) : nlohmann::json(nullptr);
    data["slotIndex"] = job->slot_index >= 0 ? nlohmann::json(job->slot_index) : nlohmann::json(nullptr);
    return command_result(job->command_id, kOpPalAdd, true, verified, false, std::move(data));
}

amity::GameResponse pal_add_place(std::shared_ptr<PalAddJob> job)
{
    UObject* world_context = any_player_controller();
    UObject* manager = character_manager(world_context);
    UObject* player_state = world_context ? player_state_by_uid(world_context, job->player_uid) : nullptr;
    const char* pawn_reason = "";
    UObject* pawn = player_state ? player_pawn(player_state, pawn_reason) : nullptr;
    UObject* container_component = pawn ? character_container_component(pawn) : nullptr;
    UObject* parameter = parameter_by_id(manager, job->player_uid, job->instance_id);

    FGuid party_container{};
    int32_t party_index = 0;
    const bool located = slot_id_of_parameter(parameter, party_container, party_index);
    job->steps["locate"] = located ? kStepOk : kStepFailed;
    if (!located || !container_component)
    {
        job->steps["place"] = kStepFailed;
        return pal_add_finish(job);
    }

    const std::string at_target = instance_id_at_target(job, world_context);
    const bool ours = job->borrowed && at_target == job->borrowed_instance;
    if (!at_target.empty() && !ours)
    {
        job->steps["place"] = kStepFailed;
        return amity::GameResponse::fail("validation_failed",
                                          "the slot was filled while the pal was being created; the new pal is in the party");
    }

    const bool swapped =
        swap_slots(container_component, party_container, party_index, job->target_container, job->target_index);
    job->steps["place"] = swapped ? kStepOk : kStepFailed;
    return amity::GameResponse::defer([job]() { return pal_add_finish(job); });
}

amity::GameResponse pal_add_wait_ready(std::shared_ptr<PalAddJob> job)
{
    UObject* world_context = any_player_controller();
    UObject* manager = character_manager(world_context);
    UObject* parameter = parameter_by_id(manager, job->player_uid, job->instance_id);
    if (!parameter)
    {
        if (++job->waited >= kReadyWaitTicks)
        {
            job->steps["ready"] = kStepFailed;
            return amity::GameResponse::fail("game_error", "the game did not finish creating the pal");
        }
        return amity::GameResponse::defer([job]() { return pal_add_wait_ready(job); });
    }
    job->steps["ready"] = kStepOk;
    job->steps["revive"] = all_steps_ok(revive_parameter(parameter)) ? kStepOk : "partial";

    UObject* handle = handle_of_parameter(manager, parameter);
    UObject* player_state = world_context ? player_state_by_uid(world_context, job->player_uid) : nullptr;
    const bool in_party = handle && give_handle_to_party(player_state, handle);
    job->steps["party"] = in_party ? kStepOk : kStepFailed;
    if (in_party)
    {
        if (job->target == AddTarget::Party)
        {
            return amity::GameResponse::defer([job]() { return pal_add_finish(job); });
        }
        return amity::GameResponse::defer([job]() { return pal_add_place(job); });
    }
    if (job->target == AddTarget::Party)
    {
        return amity::GameResponse::fail("validation_failed", "the party has no free seat");
    }
    if (job->borrowed)
    {
        return amity::GameResponse::fail("game_error", "the party took no pal even after a seat was freed");
    }

    UObject* holder = otomo_holder(player_state);
    const char* pawn_reason = "";
    UObject* pawn = player_state ? player_pawn(player_state, pawn_reason) : nullptr;
    UObject* container_component = pawn ? character_container_component(pawn) : nullptr;
    for (int32_t seat = kPartySeats - 1; seat >= 0; --seat)
    {
        UObject* party_parameter = party_parameter_at(holder, seat);
        FGuid party_container{};
        int32_t party_index = 0;
        if (!party_parameter || !slot_id_of_parameter(party_parameter, party_container, party_index))
        {
            continue;
        }
        if (!swap_slots(container_component, party_container, party_index, job->target_container, job->target_index))
        {
            break;
        }
        job->borrowed = true;
        job->borrowed_instance = instance_id_of_parameter(party_parameter);
        job->steps["borrow"] = kStepOk;
        return amity::GameResponse::defer([job]() { return pal_add_wait_ready(job); });
    }
    job->steps["borrow"] = kStepFailed;
    return amity::GameResponse::fail("game_error", "no party seat could be freed to place the new pal");
}

bool parse_slot_index(const nlohmann::json& args, const char* key, int32_t& out, std::string& error)
{
    if (!args.contains(key) || !args[key].is_number_integer())
    {
        error = std::string(key) + " is required and must be an integer";
        return false;
    }
    const int64_t value = args[key].get<int64_t>();
    if (value < kMinSlotIndex || value > kMaxSlotIndex)
    {
        error = std::string(key) + " is out of range";
        return false;
    }
    out = static_cast<int32_t>(value);
    return true;
}

nlohmann::json slot_json(int32_t slot_index, const std::string& instance_id)
{
    nlohmann::json entry = nlohmann::json::object();
    entry["slotIndex"] = slot_index;
    entry["instanceId"] = instance_id.empty() ? nlohmann::json(nullptr) : nlohmann::json(instance_id);
    return entry;
}

}

namespace amity_rt
{
amity::GameResponse command_pal_remove(const std::string& command_id, const nlohmann::json& args)
{
    FGuid player_uid{};
    std::string error{};
    if (!parse_player_uid(args, player_uid, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }

    int32_t slot_index = 0;
    if (!parse_slot_index(args, "slotIndex", slot_index, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }

    LivePlayer live{};
    if (auto failure = require_live_player(player_uid, live))
    {
        return *failure;
    }
    UObject* world_context = live.world_context;
    UObject* player_state = live.player_state;

    const char* pawn_reason = "";
    UObject* pawn = player_pawn(player_state, pawn_reason);
    if (!pawn)
    {
        return amity::GameResponse::fail("capability_unavailable", pawn_reason);
    }
    UObject* container_component = character_container_component(pawn);
    if (!container_component)
    {
        return amity::GameResponse::fail("capability_unavailable", "character container component unavailable");
    }

    std::string error_code{}, error_message{};
    UObject* slot = palbox_slot_by_index(world_context, player_uid, slot_index, error_code, error_message);
    if (!slot)
    {
        return amity::GameResponse::fail(error_code, error_message);
    }

    const std::string removed = instance_id_at(world_context, player_uid, slot_index);
    if (removed.empty() && !slot_occupied(world_context, player_uid, slot_index))
    {
        return command_result(command_id, kOpPalRemove, false, true, false, nlohmann::json{{"removed", nullptr}});
    }

    FGuid container_id{};
    int32_t addressed_index = 0;
    if (!slot_id_of(slot, container_id, addressed_index))
    {
        return amity::GameResponse::fail("capability_unavailable", "slot id unavailable");
    }

    UFunction* empty_slot_fn =
        find_function(STR("/Script/Pal.PalNetworkCharacterContainerComponent:RequestEmptySlot_ToServer_Rep"));
    if (!empty_slot_fn)
    {
        return amity::GameResponse::fail("capability_unavailable", "pal removal function unresolved");
    }

    ParamBlock block(empty_slot_fn);
    FStructProperty* slot_id_param = block.valid() ? struct_param(block, {STR("SlotId"), STR("SlotID")}) : nullptr;
    if (!slot_id_param ||
        !write_slot_id(slot_id_param->GetStruct(), block.value_of(slot_id_param), container_id, addressed_index))
    {
        return amity::GameResponse::fail("capability_unavailable", "pal removal parameters unavailable");
    }
    block.invoke(container_component);

    const bool empty_now = !slot_occupied(world_context, player_uid, slot_index);
    return command_result(command_id,
                          kOpPalRemove,
                          true,
                          empty_now,
                          false,
                          nlohmann::json{{"removed", removed.empty() ? nlohmann::json(nullptr) : nlohmann::json(removed)}});
}

amity::GameResponse command_pal_add(const std::string& command_id, const nlohmann::json& args)
{
    FGuid player_uid{};
    std::string error{};
    if (!parse_player_uid(args, player_uid, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }

    bool to_party = false;
    if (args.contains("party") && !args["party"].is_null())
    {
        if (!args["party"].is_boolean())
        {
            return amity::GameResponse::fail("validation_failed", "party must be a boolean");
        }
        to_party = args["party"].get<bool>();
    }
    std::optional<FGuid> base_id{};
    if (args.contains("baseId") && !args["baseId"].is_null())
    {
        if (!args["baseId"].is_string())
        {
            return amity::GameResponse::fail("validation_failed", "baseId must be a string");
        }
        base_id = parse_guid(args["baseId"].get<std::string>());
        if (!base_id)
        {
            return amity::GameResponse::fail("validation_failed", "baseId is not a valid GUID");
        }
    }
    if (to_party && base_id)
    {
        return amity::GameResponse::fail("validation_failed", "a pal goes to one place: baseId or party, not both");
    }

    int32_t slot_index = -1;
    if (!to_party && !parse_slot_index(args, "slotIndex", slot_index, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }

    if (!args.contains("characterId") || !args["characterId"].is_string())
    {
        return amity::GameResponse::fail("validation_failed", "characterId is required");
    }
    const std::string character_id = args["characterId"].get<std::string>();
    if (character_id.empty() || character_id.size() > kMaxCharacterIdLength)
    {
        return amity::GameResponse::fail("validation_failed", "characterId must be 1-128 characters");
    }

    int64_t level = 1;
    if (args.contains("level") && !args["level"].is_null())
    {
        if (!args["level"].is_number_integer())
        {
            return amity::GameResponse::fail("validation_failed", "level must be an integer");
        }
        level = args["level"].get<int64_t>();
        if (level < kMinLevel || level > kMaxLevel)
        {
            return amity::GameResponse::fail("validation_failed", "level must be between 1 and 255");
        }
    }

    std::string gender{};
    if (args.contains("gender") && !args["gender"].is_null())
    {
        if (!args["gender"].is_string())
        {
            return amity::GameResponse::fail("validation_failed", "gender must be a string");
        }
        gender = args["gender"].get<std::string>();
    }

    LivePlayer live{};
    if (auto failure = require_live_player(player_uid, live))
    {
        return *failure;
    }
    UObject* world_context = live.world_context;
    UObject* player_state = live.player_state;
    const char* pawn_reason = "";
    if (!player_pawn(player_state, pawn_reason))
    {
        return amity::GameResponse::fail("capability_unavailable", pawn_reason);
    }
    if (!otomo_holder(player_state))
    {
        return amity::GameResponse::fail("capability_unavailable", "party holder unavailable");
    }

    UObject* slot = nullptr;
    if (to_party)
    {
        if (!party_has_free_seat(otomo_holder(player_state)))
        {
            return amity::GameResponse::fail("validation_failed", "the party has no free seat");
        }
    }
    else if (base_id)
    {
        const char* container_reason = "";
        UObject* container = base_camp_container(*base_id, container_reason);
        if (!container)
        {
            return amity::GameResponse::fail("validation_failed", container_reason);
        }
        slot = base_container_slot(container, slot_index);
        if (!slot)
        {
            return amity::GameResponse::fail("validation_failed", "slotIndex out of range");
        }
        if (object_member(slot, STR("Handle")))
        {
            return amity::GameResponse::fail("validation_failed", "slotIndex already holds a pal");
        }
    }
    else
    {
        std::string error_code{}, error_message{};
        slot = palbox_slot_by_index(world_context, player_uid, slot_index, error_code, error_message);
        if (!slot)
        {
            return amity::GameResponse::fail(error_code, error_message);
        }
        if (slot_occupied(world_context, player_uid, slot_index))
        {
            return amity::GameResponse::fail("validation_failed", "slotIndex already holds a pal");
        }
    }

    auto job = std::make_shared<PalAddJob>();
    job->command_id = command_id;
    job->player_uid = player_uid;
    job->instance_id = random_guid();
    job->slot_index = slot_index;
    job->target = to_party ? AddTarget::Party : (base_id ? AddTarget::Base : AddTarget::Palbox);
    if (base_id)
    {
        job->base_id = *base_id;
    }
    if (slot && !slot_id_of(slot, job->target_container, job->target_index))
    {
        return amity::GameResponse::fail("capability_unavailable", "slot id unavailable");
    }

    UObject* manager = character_manager(world_context);
    UFunction* create_fn = find_function(STR("/Script/Pal.PalCharacterManager:CreateIndividualByFixedID"));
    if (!manager || !create_fn)
    {
        return amity::GameResponse::fail("capability_unavailable", "pal creation functions unresolved");
    }

    // FNAME_Find: a character id the game never interned cannot name a real pal, and looking it
    // up must not add client-supplied strings to the global name table.
    const FName character_name(widen(character_id).c_str(), FNAME_Find);
    if (character_name.IsNone())
    {
        return amity::GameResponse::fail("validation_failed", "unknown characterId");
    }

    // ParamBlock runs each parameter's real constructor, which is what makes the 880-byte save
    // parameter safe to hand over: it holds FString, TArray and TMap members that a zeroed buffer
    // would leave in a state the engine never produces. The spawn callback keeps its constructed
    // default, an unbound delegate. The id is the job's own, so the pal can be found again on
    // later ticks without keeping anything the call returned.
    ParamBlock block(create_fn);
    FStructProperty* id_param = block.valid() ? struct_param(block, {STR("ID"), STR("Id")}) : nullptr;
    FStructProperty* init_param = block.valid() ? struct_param(block, {STR("InitParameter"), STR("InitParam")}) : nullptr;
    UStruct* init_type = init_param ? init_param->GetStruct() : nullptr;
    void* init_value = init_param ? block.value_of(init_param) : nullptr;
    if (!id_param || !init_type || !init_value ||
        !write_instance_id(id_param->GetStruct(), block.value_of(id_param), player_uid, job->instance_id))
    {
        return amity::GameResponse::fail("capability_unavailable", "pal creation parameters unavailable");
    }

    if (!setup_save_parameter(world_context, character_name, level, player_uid, init_param, init_value))
    {
        return amity::GameResponse::fail("game_error", "the game could not build a parameter for that pal");
    }
    job->steps["setup"] = kStepOk;

    if (job->target == AddTarget::Party)
    {
        job->steps["slot"] = kStepSkipped;
    }
    else
    {
        auto* slot_id_prop = CastField<FStructProperty>(find_struct_prop(init_type, {STR("SlotId"), STR("SlotID")}));
        if (!slot_id_prop ||
            !write_slot_id(slot_id_prop->GetStruct(),
                            slot_id_prop->ContainerPtrToValuePtr<void>(init_value),
                            job->target_container,
                            job->target_index))
        {
            return amity::GameResponse::fail("capability_unavailable", "pal creation slot unavailable");
        }
        job->steps["slot"] = kStepOk;
    }

    if (gender.empty())
    {
        job->steps["gender"] = kStepSkipped;
    }
    else
    {
        const wchar_t* enumerator = gender == "male" || gender == "Male"     ? STR("Male")
                                     : gender == "female" || gender == "Female" ? STR("Female")
                                                                                : STR("None");
        const bool written =
            write_enum_by_name(find_struct_prop(init_type, {STR("Gender")}), init_value, enumerator);
        job->steps["gender"] = written ? kStepOk : kStepFailed;
    }

    block.invoke(manager);
    job->steps["create"] = object_return_of(block) ? kStepOk : kStepFailed;
    if (job->steps["create"] != kStepOk)
    {
        return amity::GameResponse::fail("game_error", "the game created no pal");
    }

    return amity::GameResponse::defer([job]() { return pal_add_wait_ready(job); });
}

amity::GameResponse command_pal_move(const std::string& command_id, const nlohmann::json& args)
{
    FGuid player_uid{};
    std::string error{};
    if (!parse_player_uid(args, player_uid, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }

    int32_t from_index = 0;
    int32_t to_index = 0;
    if (!parse_slot_index(args, "fromSlotIndex", from_index, error) ||
        !parse_slot_index(args, "toSlotIndex", to_index, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }
    if (from_index == to_index)
    {
        return amity::GameResponse::fail("validation_failed", "fromSlotIndex and toSlotIndex are the same slot");
    }

    LivePlayer live{};
    if (auto failure = require_live_player(player_uid, live))
    {
        return *failure;
    }
    UObject* world_context = live.world_context;
    UObject* player_state = live.player_state;

    const char* pawn_reason = "";
    UObject* pawn = player_pawn(player_state, pawn_reason);
    if (!pawn)
    {
        return amity::GameResponse::fail("capability_unavailable", pawn_reason);
    }
    UObject* container_component = character_container_component(pawn);
    if (!container_component)
    {
        return amity::GameResponse::fail("capability_unavailable", "character container component unavailable");
    }

    std::string error_code{}, error_message{};
    UObject* from_slot = palbox_slot_by_index(world_context, player_uid, from_index, error_code, error_message);
    if (!from_slot)
    {
        return amity::GameResponse::fail(error_code, error_message);
    }
    UObject* to_slot = palbox_slot_by_index(world_context, player_uid, to_index, error_code, error_message);
    if (!to_slot)
    {
        return amity::GameResponse::fail(error_code, error_message);
    }

    const std::string moving = instance_id_at(world_context, player_uid, from_index);
    if (moving.empty())
    {
        return amity::GameResponse::fail("validation_failed", "fromSlotIndex holds no pal");
    }
    const std::string displaced = instance_id_at(world_context, player_uid, to_index);

    FGuid from_container{};
    FGuid to_container{};
    int32_t from_addressed = 0;
    int32_t to_addressed = 0;
    if (!slot_id_of(from_slot, from_container, from_addressed) || !slot_id_of(to_slot, to_container, to_addressed))
    {
        return amity::GameResponse::fail("capability_unavailable", "slot id unavailable");
    }

    if (!swap_slots(container_component, from_container, from_addressed, to_container, to_addressed))
    {
        return amity::GameResponse::fail("capability_unavailable", "pal move parameters unavailable");
    }

    const std::string now_at_target = instance_id_at(world_context, player_uid, to_index);
    const std::string now_at_source = instance_id_at(world_context, player_uid, from_index);
    const bool verified = now_at_target == moving && now_at_source == displaced;

    nlohmann::json data = nlohmann::json::object();
    data["from"] = slot_json(from_index, now_at_source);
    data["to"] = slot_json(to_index, now_at_target);
    return command_result(command_id, kOpPalMove, true, verified, false, std::move(data));
}
}
