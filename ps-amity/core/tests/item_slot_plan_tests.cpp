#include <amity/item_slot_plan.hpp>

#include <doctest/doctest.h>

using namespace amity;

namespace
{
constexpr const char* kCommon = "11111111-1111-1111-1111-111111111111";
constexpr const char* kEssential = "22222222-2222-2222-2222-222222222222";

nlohmann::json slot(int32_t index, const char* item_id, const nlohmann::json& count)
{
    return nlohmann::json{{"slotIndex", index}, {"staticItemId", item_id}, {"count", count}};
}

nlohmann::json containers(nlohmann::json common_slots, nlohmann::json essential_slots = nlohmann::json::array())
{
    return nlohmann::json::array({
        nlohmann::json{{"containerId", kCommon}, {"slots", std::move(common_slots)}},
        nlohmann::json{{"containerId", kEssential}, {"slots", std::move(essential_slots)}},
    });
}
}

TEST_CASE("read_slot reports an absent slot as an empty one")
{
    const nlohmann::json state = containers(nlohmann::json::array({slot(3, "Wood", 10)}));

    const SlotContents empty = read_slot(state, kCommon, 7);
    CHECK(empty.container_found);
    CHECK_FALSE(empty.degraded);
    CHECK(empty.static_item_id.empty());
    CHECK(empty.count == 0);

    const SlotContents held = read_slot(state, kCommon, 3);
    CHECK(held.static_item_id == "Wood");
    CHECK(held.count == 10);
}

TEST_CASE("read_slot distinguishes an unknown container from an empty slot")
{
    const nlohmann::json state = containers(nlohmann::json::array());
    CHECK_FALSE(read_slot(state, "no-such-container", 0).container_found);
    CHECK(read_slot(state, kCommon, 0).container_found);
}

TEST_CASE("read_slot flags a slot whose count could not be read")
{
    const nlohmann::json state = containers(nlohmann::json::array({slot(0, "Wood", nullptr)}));
    const SlotContents contents = read_slot(state, kCommon, 0);
    CHECK(contents.degraded);
    CHECK(contents.static_item_id == "Wood");
}

TEST_CASE("plan_set_slot fills an empty slot by granting the whole amount")
{
    SlotContents empty{};
    empty.container_found = true;
    const SetSlotPlan plan = plan_set_slot(empty, "Wood", 20);
    CHECK(plan.grant_count == 20);
    CHECK(plan.dispose_count == 0);
}

TEST_CASE("plan_set_slot grants only the shortfall when the slot already holds the item")
{
    SlotContents held{};
    held.container_found = true;
    held.static_item_id = "Wood";
    held.count = 8;

    const SetSlotPlan plan = plan_set_slot(held, "Wood", 20);
    CHECK(plan.grant_count == 12);
    CHECK(plan.dispose_count == 0);
}

TEST_CASE("plan_set_slot disposes the surplus when fewer are wanted")
{
    SlotContents held{};
    held.container_found = true;
    held.static_item_id = "Wood";
    held.count = 20;

    const SetSlotPlan plan = plan_set_slot(held, "Wood", 8);
    CHECK(plan.dispose_count == 12);
    CHECK(plan.grant_count == 0);
}

TEST_CASE("plan_set_slot does nothing when the slot already matches")
{
    SlotContents held{};
    held.container_found = true;
    held.static_item_id = "Wood";
    held.count = 20;
    CHECK(plan_set_slot(held, "Wood", 20).no_op());
}

TEST_CASE("plan_set_slot clears the slot before granting a different item")
{
    SlotContents held{};
    held.container_found = true;
    held.static_item_id = "Wood";
    held.count = 20;

    const SetSlotPlan plan = plan_set_slot(held, "Stone", 5);
    CHECK(plan.dispose_count == 20);
    CHECK(plan.grant_count == 5);
}

TEST_CASE("plan_set_slot empties the slot when nothing is wanted")
{
    SlotContents held{};
    held.container_found = true;
    held.static_item_id = "Wood";
    held.count = 20;

    CHECK(plan_set_slot(held, "Wood", 0).dispose_count == 20);
    CHECK(plan_set_slot(held, "", 5).dispose_count == 20);
    CHECK(plan_set_slot(held, "Wood", 0).grant_count == 0);
}

TEST_CASE("plan_set_slot leaves an already empty slot alone when nothing is wanted")
{
    SlotContents empty{};
    empty.container_found = true;
    CHECK(plan_set_slot(empty, "", 0).no_op());
}

TEST_CASE("gained_slots finds a stack that appeared where there was none")
{
    const nlohmann::json before = containers(nlohmann::json::array());
    const nlohmann::json after = containers(nlohmann::json::array({slot(4, "Wood", 20)}));

    const std::vector<SlotGain> gains = gained_slots(before, after, "Wood");
    REQUIRE(gains.size() == 1);
    CHECK(gains[0].container_id == kCommon);
    CHECK(gains[0].slot_index == 4);
    CHECK(gains[0].gained == 20);
}

TEST_CASE("gained_slots reports only the increase on a stack that was topped up")
{
    const nlohmann::json before = containers(nlohmann::json::array({slot(4, "Wood", 30)}));
    const nlohmann::json after = containers(nlohmann::json::array({slot(4, "Wood", 50)}));

    const std::vector<SlotGain> gains = gained_slots(before, after, "Wood");
    REQUIRE(gains.size() == 1);
    CHECK(gains[0].gained == 20);
}

TEST_CASE("gained_slots spans containers and several slots at once")
{
    const nlohmann::json before = containers(nlohmann::json::array({slot(0, "Wood", 99)}));
    const nlohmann::json after = containers(nlohmann::json::array({slot(0, "Wood", 100), slot(1, "Wood", 40)}),
                                             nlohmann::json::array({slot(2, "Wood", 10)}));

    const std::vector<SlotGain> gains = gained_slots(before, after, "Wood");
    REQUIRE(gains.size() == 3);
    int64_t total = 0;
    for (const SlotGain& gain : gains)
    {
        total += gain.gained;
    }
    CHECK(total == 51);
}

TEST_CASE("gained_slots ignores other items and slots that did not grow")
{
    const nlohmann::json before = containers(nlohmann::json::array({slot(0, "Wood", 10), slot(1, "Stone", 5)}));
    const nlohmann::json after = containers(nlohmann::json::array({slot(0, "Wood", 10), slot(1, "Stone", 90)}));

    CHECK(gained_slots(before, after, "Wood").empty());
}

TEST_CASE("gained_slots counts the whole stack when a slot changed item")
{
    const nlohmann::json before = containers(nlohmann::json::array({slot(0, "Stone", 40)}));
    const nlohmann::json after = containers(nlohmann::json::array({slot(0, "Wood", 12)}));

    const std::vector<SlotGain> gains = gained_slots(before, after, "Wood");
    REQUIRE(gains.size() == 1);
    CHECK(gains[0].gained == 12);
}

TEST_CASE("gained_slots skips a slot whose count could not be read")
{
    const nlohmann::json before = containers(nlohmann::json::array());
    const nlohmann::json after = containers(nlohmann::json::array({slot(0, "Wood", nullptr)}));

    CHECK(gained_slots(before, after, "Wood").empty());
}
