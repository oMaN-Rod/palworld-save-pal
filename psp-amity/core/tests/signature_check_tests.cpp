#include <doctest/doctest.h>

#include <signature_check.hpp>

#include <iterator>
#include <string>
#include <vector>

namespace {

const amity_sig::FunctionSpec& function_of(const std::string& op, const std::string& display) {
    const amity_sig::OpSpec* spec = amity_sig::find_op_spec(op);
    REQUIRE(spec != nullptr);
    for (std::size_t i = 0; i < spec->function_count; ++i) {
        if (display == spec->functions[i].display) {
            return spec->functions[i];
        }
    }
    FAIL("no such function in op table: ", display);
    return spec->functions[0];
}

std::vector<amity_sig::ObservedParam> observed_of(const amity_sig::FunctionSpec& spec) {
    std::vector<amity_sig::ObservedParam> observed;
    for (std::size_t i = 0; i < spec.param_count; ++i) {
        observed.push_back({spec.params[i].name, spec.params[i].type, spec.params[i].size});
    }
    return observed;
}

}

TEST_CASE("the table covers exactly the write ops the bridge dispatches") {
    std::size_t count = 0;
    const amity_sig::OpSpec* specs = amity_sig::op_specs(count);
    REQUIRE(count == 9);
    const char* expected[] = {
        "pal.heal", "item.setSlot", "pal.remove", "pal.move", "pal.add",
        "pal.edit", "player.edit",  "guild.edit", "guild.setRole",
    };
    for (std::size_t i = 0; i < count; ++i) {
        CAPTURE(i);
        CHECK(std::string(specs[i].op) == expected[i]);
    }
    CHECK(amity_sig::find_op_spec("pal.heal") == &specs[0]);
    CHECK(amity_sig::find_op_spec("player.heal") == nullptr);
    CHECK(amity_sig::find_op_spec("") == nullptr);
}

TEST_CASE("AddItem_ServerInternal's table entry matches the live parameter dump") {
    const amity_sig::FunctionSpec& spec = function_of("item.setSlot", "AddItem_ServerInternal");
    REQUIRE(spec.param_count == 6);
    const amity_sig::ParamSpec expected[] = {
        {L"StaticItemId", L"NameProperty", 8}, {L"Count", L"IntProperty", 4},
        {L"IsAssignPassive", L"BoolProperty", 1}, {L"LogDelay", L"FloatProperty", 4},
        {L"bNotifyLog", L"BoolProperty", 1}, {L"ReturnValue", L"EnumProperty", 1},
    };
    for (std::size_t i = 0; i < spec.param_count; ++i) {
        CAPTURE(i);
        CHECK(std::wstring(spec.params[i].name) == expected[i].name);
        CHECK(std::wstring(spec.params[i].type) == expected[i].type);
        CHECK(spec.params[i].size == expected[i].size);
    }
}

TEST_CASE("every table function's own params are an exact match") {
    std::size_t count = 0;
    const amity_sig::OpSpec* specs = amity_sig::op_specs(count);
    for (std::size_t i = 0; i < count; ++i) {
        for (std::size_t f = 0; f < specs[i].function_count; ++f) {
            const amity_sig::FunctionSpec& spec = specs[i].functions[f];
            CAPTURE(spec.display);
            std::wstring detail;
            CHECK(amity_sig::params_match(spec, observed_of(spec), detail));
            CHECK(detail.empty());
        }
    }
}

TEST_CASE("a no-parameter function matches only an empty chain") {
    const amity_sig::FunctionSpec& spec = function_of("pal.heal", "FullRecoveryHP");
    REQUIRE(spec.param_count == 0);
    std::wstring detail;
    CHECK(amity_sig::params_match(spec, {}, detail));
    CHECK_FALSE(amity_sig::params_match(spec, {{L"ReturnValue", L"BoolProperty", 1}}, detail));
    CHECK(detail.find(L"found 1") != std::wstring::npos);
}

TEST_CASE("a missing parameter fails on the count") {
    const amity_sig::FunctionSpec& spec = function_of("item.setSlot", "AddItem_ServerInternal");
    auto observed = observed_of(spec);
    observed.pop_back();
    std::wstring detail;
    CHECK_FALSE(amity_sig::params_match(spec, observed, detail));
    CHECK(detail.find(L"expected 6 params") != std::wstring::npos);
}

TEST_CASE("an extra parameter fails on the count") {
    const amity_sig::FunctionSpec& spec = function_of("pal.add", "CreateIndividualByFixedID");
    auto observed = observed_of(spec);
    observed.push_back({L"Extra", L"IntProperty", 4});
    std::wstring detail;
    CHECK_FALSE(amity_sig::params_match(spec, observed, detail));
}

TEST_CASE("a renamed parameter is a mismatch naming both spellings") {
    const amity_sig::FunctionSpec& spec = function_of("pal.add", "SetupSaveParameter");
    auto observed = observed_of(spec);
    observed[0].name = L"characterID";
    std::wstring detail;
    CHECK_FALSE(amity_sig::params_match(spec, observed, detail));
    CHECK(detail.find(L"CharacterID") != std::wstring::npos);
    CHECK(detail.find(L"characterID") != std::wstring::npos);
}

TEST_CASE("a changed property class is a mismatch") {
    const amity_sig::FunctionSpec& spec = function_of("item.setSlot", "AddItem_ServerInternal");
    auto observed = observed_of(spec);
    observed[0].type = L"StrProperty";
    std::wstring detail;
    CHECK_FALSE(amity_sig::params_match(spec, observed, detail));
    CHECK(detail.find(L"StaticItemId") != std::wstring::npos);
    CHECK(detail.find(L"StrProperty") != std::wstring::npos);
}

TEST_CASE("a changed parameter size is a mismatch even when name and class hold") {
    const amity_sig::FunctionSpec& spec = function_of("item.setSlot", "AddItem_ServerInternal");
    auto observed = observed_of(spec);
    observed[1].size = 8;
    std::wstring detail;
    CHECK_FALSE(amity_sig::params_match(spec, observed, detail));
    CHECK(detail.find(L"expected size 4") != std::wstring::npos);
}

TEST_CASE("reordered parameters are a mismatch") {
    const amity_sig::FunctionSpec& spec = function_of("item.setSlot", "AddItem_ServerInternal");
    auto observed = observed_of(spec);
    std::swap(observed[3], observed[4]);
    std::wstring detail;
    CHECK_FALSE(amity_sig::params_match(spec, observed, detail));
}

TEST_CASE("pal.heal requires all six parameter setters and readers") {
    const amity_sig::OpSpec* spec = amity_sig::find_op_spec("pal.heal");
    REQUIRE(spec != nullptr);
    REQUIRE(spec->function_count >= 6);
    for (std::size_t i = 0; i < 6; ++i) {
        CAPTURE(i);
        CHECK(std::wstring(spec->functions[i].path).rfind(L"/Script/Pal.PalIndividualCharacterParameter:", 0) == 0);
    }
}

TEST_CASE("pal.heal also requires the box-to-parameter resolution chain") {
    const amity_sig::OpSpec* spec = amity_sig::find_op_spec("pal.heal");
    REQUIRE(spec != nullptr);
    REQUIRE(spec->function_count == 10);
    const std::wstring expected[] = {
        L"/Script/Pal.PalUtility:GetPalStorageDataByPlayerUID",
        L"/Script/Pal.PalPlayerDataPalStorage:GetSlotBySlotIndex",
        L"/Script/Pal.PalIndividualCharacterSlot:GetHandle",
        L"/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter",
    };
    for (std::size_t i = 0; i < std::size(expected); ++i) {
        CAPTURE(i);
        CHECK(std::wstring(spec->functions[6 + i].path) == expected[i]);
    }
}

TEST_CASE("the pawn-addressed ops resolve the player controller and pawn") {
    for (const char* op : {"item.setSlot", "pal.remove", "pal.move"}) {
        CAPTURE(op);
        const amity_sig::FunctionSpec& controller = function_of(op, "GetPlayerController");
        REQUIRE(controller.param_count == 1);
        CHECK(std::wstring(controller.params[0].name) == L"ReturnValue");
        CHECK(std::wstring(controller.params[0].type) == L"ObjectProperty");
        CHECK(controller.params[0].size == 8);

        const amity_sig::FunctionSpec& pawn = function_of(op, "K2_GetPawn");
        CHECK(std::wstring(pawn.path) == L"/Script/Engine.Controller:K2_GetPawn");
        REQUIRE(pawn.param_count == 1);
        CHECK(std::wstring(pawn.params[0].name) == L"ReturnValue");
        CHECK(std::wstring(pawn.params[0].type) == L"ObjectProperty");
    }
}

TEST_CASE("every player-addressed op covers the player-state lookup they share") {
    for (const char* op : {"item.setSlot", "pal.remove", "pal.move", "pal.add", "pal.edit", "player.edit"}) {
        CAPTURE(op);
        const amity_sig::FunctionSpec& spec = function_of(op, "GetAllPlayerStates");
        CHECK(std::wstring(spec.path) == L"/Script/Pal.PalUtility:GetAllPlayerStates");
        REQUIRE(spec.param_count == 2);
        CHECK(std::wstring(spec.params[0].name) == L"WorldContextObject");
        CHECK(std::wstring(spec.params[0].type) == L"ObjectProperty");
        CHECK(spec.params[0].size == 8);
        CHECK(std::wstring(spec.params[1].name) == L"OutPlayerStates");
        CHECK(std::wstring(spec.params[1].type) == L"ArrayProperty");
        CHECK(spec.params[1].size == 16);
    }
}

TEST_CASE("player.edit covers the character manager its save parameter hangs off") {
    const amity_sig::FunctionSpec& spec = function_of("player.edit", "GetCharacterManager");
    CHECK(std::wstring(spec.path) == L"/Script/Pal.PalUtility:GetCharacterManager");
    REQUIRE(spec.param_count == 2);
    CHECK(std::wstring(spec.params[0].name) == L"WorldContextObject");
    CHECK(std::wstring(spec.params[1].name) == L"ReturnValue");
    CHECK(std::wstring(spec.params[1].type) == L"ObjectProperty");
}

TEST_CASE("the read table holds the snapshot reads and never leaks into the write table") {
    std::size_t count = 0;
    const amity_sig::OpSpec* specs = amity_sig::read_op_specs(count);
    REQUIRE(count == 3);
    CHECK(std::string(specs[0].op) == "inventory");
    CHECK(std::string(specs[1].op) == "party");
    CHECK(std::string(specs[2].op) == "palSlot");
    CHECK(amity_sig::find_read_op_spec("inventory") == &specs[0]);
    CHECK(amity_sig::find_read_op_spec("party") == &specs[1]);
    CHECK(amity_sig::find_read_op_spec("palSlot") == &specs[2]);
    CHECK(amity_sig::find_read_op_spec("item.setSlot") == nullptr);
    CHECK(amity_sig::find_op_spec("inventory") == nullptr);
    CHECK(amity_sig::find_op_spec("party") == nullptr);
}

TEST_CASE("the pal slot id return is pinned to the container-plus-index struct") {
    const amity_sig::OpSpec* spec = amity_sig::find_read_op_spec("palSlot");
    REQUIRE(spec != nullptr);
    REQUIRE(spec->function_count == 1);
    const amity_sig::FunctionSpec& fn_spec = spec->functions[0];
    REQUIRE(fn_spec.param_count == 1);
    CHECK(std::wstring(fn_spec.params[0].name) == L"ReturnValue");
    CHECK(std::wstring(fn_spec.params[0].type) == L"StructProperty");
    CHECK(fn_spec.params[0].size == 20);
}

TEST_CASE("the party read pins the pawn-to-holder chain") {
    const amity_sig::OpSpec* spec = amity_sig::find_read_op_spec("party");
    REQUIRE(spec != nullptr);
    REQUIRE(spec->function_count == 5);
    const std::wstring expected[] = {
        L"/Script/Engine.PlayerState:GetPlayerController",
        L"/Script/Engine.Controller:K2_GetPawn",
        L"/Script/Engine.Actor:GetComponentByClass",
        L"/Script/Pal.PalOtomoHolderComponentBase:GetOtomoIndividualHandle",
        L"/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter",
    };
    for (std::size_t i = 0; i < std::size(expected); ++i) {
        CAPTURE(i);
        CHECK(std::wstring(spec->functions[i].path) == expected[i]);
    }
}

TEST_CASE("GetComponentByClass takes the component class, not an object") {
    const amity_sig::OpSpec* spec = amity_sig::find_read_op_spec("party");
    REQUIRE(spec != nullptr);
    REQUIRE(spec->function_count == 5);
    const amity_sig::FunctionSpec& fn_spec = spec->functions[2];
    REQUIRE(fn_spec.param_count == 2);
    CHECK(std::wstring(fn_spec.params[0].name) == L"ComponentClass");
    CHECK(std::wstring(fn_spec.params[0].type) == L"ClassProperty");
    CHECK(fn_spec.params[0].size == 8);
    CHECK(std::wstring(fn_spec.params[1].name) == L"ReturnValue");
    CHECK(std::wstring(fn_spec.params[1].type) == L"ObjectProperty");
}

TEST_CASE("the otomo handle accessor is addressed by slot index") {
    const amity_sig::OpSpec* spec = amity_sig::find_read_op_spec("party");
    REQUIRE(spec != nullptr);
    REQUIRE(spec->function_count == 5);
    const amity_sig::FunctionSpec& fn_spec = spec->functions[3];
    REQUIRE(fn_spec.param_count == 2);
    CHECK(std::wstring(fn_spec.params[0].name) == L"SlotIndex");
    CHECK(std::wstring(fn_spec.params[0].type) == L"IntProperty");
    CHECK(fn_spec.params[0].size == 4);
}

TEST_CASE("the inventory read pins the three calls whose layouts were dumped live") {
    const amity_sig::OpSpec* spec = amity_sig::find_read_op_spec("inventory");
    REQUIRE(spec != nullptr);
    REQUIRE(spec->function_count == 3);
    CHECK(std::wstring(spec->functions[0].path) == L"/Script/Pal.PalUtility:GetAllPlayerStates");
    CHECK(std::wstring(spec->functions[1].path) == L"/Script/Pal.PalPlayerState:GetInventoryData");
    REQUIRE(spec->functions[1].param_count == 1);
    CHECK(std::wstring(spec->functions[1].params[0].name) == L"ReturnValue");
    CHECK(std::wstring(spec->functions[1].params[0].type) == L"ObjectProperty");
    CHECK(spec->functions[1].params[0].size == 8);
    CHECK(std::wstring(spec->functions[2].path) ==
          L"/Script/Pal.PalPlayerInventoryData:TryGetContainerFromInventoryType");
}

TEST_CASE("TryGetContainerFromInventoryType's table entry matches the live parameter dump") {
    const amity_sig::OpSpec* spec = amity_sig::find_read_op_spec("inventory");
    REQUIRE(spec != nullptr);
    REQUIRE(spec->function_count == 3);
    const amity_sig::FunctionSpec& fn_spec = spec->functions[2];
    CHECK(std::string(fn_spec.display) == "TryGetContainerFromInventoryType");
    REQUIRE(fn_spec.param_count == 3);
    const amity_sig::ParamSpec expected[] = {
        {L"inventoryType", L"EnumProperty", 1},
        {L"OutContainer", L"ObjectProperty", 8},
        {L"ReturnValue", L"BoolProperty", 1},
    };
    for (std::size_t i = 0; i < fn_spec.param_count; ++i) {
        CAPTURE(i);
        CHECK(std::wstring(fn_spec.params[i].name) == expected[i].name);
        CHECK(std::wstring(fn_spec.params[i].type) == expected[i].type);
        CHECK(fn_spec.params[i].size == expected[i].size);
    }
}

TEST_CASE("GetIndividualCharacterParameter's table entry matches the live parameter dump") {
    const amity_sig::FunctionSpec& spec = function_of("player.edit", "GetIndividualCharacterParameter");
    CHECK(std::wstring(spec.path) == L"/Script/Pal.PalCharacterManager:GetIndividualCharacterParameter");
    REQUIRE(spec.param_count == 2);
    const amity_sig::ParamSpec expected[] = {
        {L"IndividualId", L"StructProperty", 48},
        {L"ReturnValue", L"ObjectProperty", 8},
    };
    for (std::size_t i = 0; i < spec.param_count; ++i) {
        CAPTURE(i);
        CHECK(std::wstring(spec.params[i].name) == expected[i].name);
        CHECK(std::wstring(spec.params[i].type) == expected[i].type);
        CHECK(spec.params[i].size == expected[i].size);
    }
}

TEST_CASE("every read-table function's own params are an exact match") {
    std::size_t count = 0;
    const amity_sig::OpSpec* specs = amity_sig::read_op_specs(count);
    for (std::size_t i = 0; i < count; ++i) {
        for (std::size_t f = 0; f < specs[i].function_count; ++f) {
            const amity_sig::FunctionSpec& spec = specs[i].functions[f];
            CAPTURE(spec.display);
            std::wstring detail;
            CHECK(amity_sig::params_match(spec, observed_of(spec), detail));
            CHECK(detail.empty());
        }
    }
}
