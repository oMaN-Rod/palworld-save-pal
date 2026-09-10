#include "game_commands.hpp"

#include "command_common.hpp"
#include "game_call.hpp"
#include "reflect.hpp"
#include "signature_check.hpp"
#include "snapshots.hpp"

#include <DynamicOutput/DynamicOutput.hpp>
#include <Unreal/FField.hpp>

#include <map>
#include <optional>
#include <utility>

using namespace RC;
using namespace RC::Unreal;
using namespace amity_rt::snap;

namespace
{
constexpr const wchar_t* kItemResultEnum = STR("/Script/Pal.EPalItemOperationResult");

struct FloatReturn
{
    float ReturnValue{};
};

struct BoolReturn
{
    bool ReturnValue{};
};

// Same blind spot for every TArray parameter: ArrayProperty(16) says nothing about the element
// the engine writes into the out-param. We read it back as 8-byte UObject*, so the inner has to
// be an object pointer of that stride whose declared class is a PlayerState.
bool out_player_states_holds_player_state_pointers(UFunction* fn)
{
    auto* array_prop = CastField<FArrayProperty>(amity_rt::find_struct_prop(fn, {STR("OutPlayerStates")}));
    if (!array_prop)
    {
        return false;
    }
    auto* inner = CastField<FObjectProperty>(array_prop->GetInner());
    if (!inner || inner->GetSize() != static_cast<int32_t>(sizeof(UObject*)))
    {
        return false;
    }
    UClass* element_class = inner->GetPropertyClass().Get();
    UClass* player_state_class = amity_rt::find_class(STR("/Script/Engine.PlayerState"));
    return element_class && player_state_class && element_class->IsChildOf(player_state_class);
}

std::vector<amity_sig::ObservedParam> describe_params(UFunction* fn)
{
    std::vector<amity_sig::ObservedParam> params{};
    for (FProperty* prop : TFieldRange<FProperty>(fn, EFieldIterationFlags::None))
    {
        if (!prop || !prop->HasAnyPropertyFlags(CPF_Parm))
        {
            continue;
        }
        amity_sig::ObservedParam observed{};
        observed.name = prop->GetName();
        observed.type = prop->GetClass().GetName();
        observed.size = prop->GetSize();
        params.push_back(std::move(observed));
    }
    return params;
}

struct SignatureVerdict
{
    bool terminal{false};
    bool ok{false};
    std::string reason{"not validated"};
};

std::map<std::string, SignatureVerdict>& signature_verdicts()
{
    static std::map<std::string, SignatureVerdict> verdicts{};
    return verdicts;
}

void refuse(SignatureVerdict& verdict, const char* display)
{
    Output::send<LogLevel::Warning>(STR("[PSAmity] param layout mismatch {}\n"), amity_rt::widen(display));
    verdict.terminal = true;
    verdict.ok = false;
    verdict.reason = std::string("signature mismatch: ") + display;
}

void validate_op(const amity_sig::OpSpec& spec, SignatureVerdict& verdict)
{
    std::map<std::string, UFunction*> resolved{};
    for (std::size_t i = 0; i < spec.function_count; ++i)
    {
        const amity_sig::FunctionSpec& fn_spec = spec.functions[i];
        UFunction* fn = amity_rt::find_function(fn_spec.path);
        if (!fn)
        {
            verdict.ok = false;
            verdict.reason = std::string("unresolved: ") + fn_spec.display;
            return;
        }
        std::wstring detail{};
        if (!amity_sig::params_match(fn_spec, describe_params(fn), detail))
        {
            Output::send<LogLevel::Warning>(STR("[PSAmity] signature mismatch {} ({})\n"), amity_rt::widen(fn_spec.display), detail);
            verdict.terminal = true;
            verdict.ok = false;
            verdict.reason = std::string("signature mismatch: ") + fn_spec.display;
            return;
        }
        resolved[fn_spec.display] = fn;
    }

    auto matched = [&resolved](const char* display) -> UFunction* {
        auto entry = resolved.find(display);
        return entry == resolved.end() ? nullptr : entry->second;
    };

    if (UFunction* all_player_states = matched("GetAllPlayerStates");
        all_player_states && !out_player_states_holds_player_state_pointers(all_player_states))
    {
        refuse(verdict, "GetAllPlayerStates");
        return;
    }

    if (std::string(spec.op) == amity_rt::kOpItemSetSlot)
    {
        if (!amity_rt::find_enum(kItemResultEnum))
        {
            verdict.ok = false;
            verdict.reason = "unresolved: EPalItemOperationResult";
            return;
        }
        if (!amity_rt::add_item_layout_matches(matched("AddItem_ServerInternal")))
        {
            refuse(verdict, "AddItem_ServerInternal");
            return;
        }
    }

    verdict.terminal = true;
    verdict.ok = true;
    verdict.reason.clear();
}

bool signature_ok_for(const amity_sig::OpSpec* spec, const std::string& op, std::string& reason)
{
    if (!spec)
    {
        reason = "unknown capability";
        return false;
    }
    SignatureVerdict& verdict = signature_verdicts()[op];
    if (!verdict.terminal)
    {
        validate_op(*spec, verdict);
    }
    reason = verdict.reason;
    return verdict.ok;
}
}

namespace amity_rt
{
bool is_write_op(const std::string& op)
{
    return amity_sig::find_op_spec(op) != nullptr;
}

bool op_signature_ok(const std::string& op, std::string& reason)
{
    return signature_ok_for(amity_sig::find_op_spec(op), op, reason);
}

bool read_op_signature_ok(const std::string& op, std::string& reason)
{
    return signature_ok_for(amity_sig::find_read_op_spec(op), op, reason);
}

void seed_capabilities(amity::CapabilityRegistry& registry)
{
    std::size_t count = 0;
    const amity_sig::OpSpec* specs = amity_sig::op_specs(count);
    for (std::size_t i = 0; i < count; ++i)
    {
        registry.set(specs[i].op, false, "not authoritative");
    }
}

void refresh_capabilities(amity::CapabilityRegistry& registry)
{
    static std::map<std::string, std::pair<bool, std::string>> logged{};

    std::string mode{};
    bool authoritative = false;
    resolve_mode(mode, authoritative);

    std::size_t count = 0;
    const amity_sig::OpSpec* specs = amity_sig::op_specs(count);
    for (std::size_t i = 0; i < count; ++i)
    {
        const std::string op = specs[i].op;
        std::string reason{};
        const bool signature_ok = op_signature_ok(op, reason);
        const bool available = authoritative && signature_ok;
        std::string effective_reason = available ? std::string() : (authoritative ? reason : std::string("not authoritative"));

        auto entry = logged.find(op);
        if (entry == logged.end() || entry->second.first != available || entry->second.second != effective_reason)
        {
            if (available)
            {
                Output::send<LogLevel::Verbose>(STR("[PSAmity] capability {} -> available\n"), widen(op));
            }
            else
            {
                Output::send<LogLevel::Verbose>(STR("[PSAmity] capability {} -> unavailable ({})\n"),
                                                 widen(op),
                                                 widen(effective_reason));
            }
            logged[op] = std::make_pair(available, effective_reason);
        }

        registry.set(op, available, effective_reason);
    }
}

amity::GameResponse command_pal_heal(const std::string& command_id, const nlohmann::json& args)
{
    FGuid player_uid{};
    std::string error{};
    if (!parse_player_uid(args, player_uid, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }
    int64_t slot_index = 0;
    if (!parse_bounded_int(args, "slotIndex", 0, 65535, slot_index, error))
    {
        return amity::GameResponse::fail("validation_failed", error);
    }

    UObject* world_context = any_player_controller();
    if (!world_context)
    {
        return amity::GameResponse::fail("capability_unavailable", "no live world context");
    }

    std::string error_code{}, error_message{};
    UObject* parameter = individual_parameter_by_slot_index(world_context,
                                                             player_uid,
                                                             static_cast<int32_t>(slot_index),
                                                             error_code,
                                                             error_message);
    if (!parameter)
    {
        return amity::GameResponse::fail(error_code, error_message);
    }

    const SaveParameterView save = save_parameter_of(parameter);

    nlohmann::json steps = revive_parameter(parameter);
    bool any_step_ran = false;
    for (const auto& [name, state] : steps.items())
    {
        any_step_ran = any_step_ran || state == kStepOk;
    }

    bool verified = false;
    if (steps["hp"] == kStepOk)
    {
        UFunction* is_recovered_fn = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:IsHPFullRecovered"));
        if (is_recovered_fn)
        {
            BoolReturn recovered{};
            parameter->ProcessEvent(is_recovered_fn, &recovered);
            verified = recovered.ReturnValue;
            if (!verified)
            {
                steps["hp"] = kStepFailed;
            }
        }
    }

    UFunction* get_max_sanity_fn = find_function(STR("/Script/Pal.PalIndividualCharacterParameter:GetMaxSanityValue"));
    FProperty* sanity_prop = find_struct_prop(save.type, {STR("SanityValue")});
    if (get_max_sanity_fn && sanity_prop && save.ptr)
    {
        FloatReturn max_sanity{};
        parameter->ProcessEvent(get_max_sanity_fn, &max_sanity);
        const bool written = write_float(sanity_prop, save.ptr, max_sanity.ReturnValue);
        steps["sanity"] = written ? kStepOk : kStepFailed;
        any_step_ran = any_step_ran || written;
    }
    else
    {
        steps["sanity"] = kStepUnresolved;
    }

    FProperty* worker_sick_prop = find_struct_prop(save.type, {STR("WorkerSick")});
    FProperty* hunger_prop = find_struct_prop(save.type, {STR("HungerType")});
    if ((worker_sick_prop || hunger_prop) && save.ptr)
    {
        const bool worker_written = write_integral(worker_sick_prop, save.ptr, 0);
        const bool hunger_written = write_integral(hunger_prop, save.ptr, 0);
        steps["workerSick"] = (worker_written && hunger_written) ? kStepOk : kStepFailed;
        any_step_ran = any_step_ran || worker_written || hunger_written;
    }
    else
    {
        steps["workerSick"] = kStepUnresolved;
    }

    return command_result(command_id, kOpPalHeal, any_step_ran, verified, true, nlohmann::json{{"steps", steps}});
}
}
