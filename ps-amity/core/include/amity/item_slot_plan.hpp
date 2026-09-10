#pragma once

#include <nlohmann/json.hpp>

#include <cstdint>
#include <string>
#include <vector>

namespace amity
{
struct SlotContents
{
    bool container_found{false};
    bool degraded{false};
    std::string static_item_id{};
    int64_t count{0};
};

struct SlotGain
{
    std::string container_id{};
    int32_t slot_index{0};
    int64_t gained{0};
};

struct SetSlotPlan
{
    int64_t dispose_count{0};
    int64_t grant_count{0};

    bool no_op() const { return dispose_count == 0 && grant_count == 0; }
};

SlotContents read_slot(const nlohmann::json& containers, const std::string& container_id, int32_t slot_index);

std::vector<SlotGain> gained_slots(const nlohmann::json& before,
                                    const nlohmann::json& after,
                                    const std::string& static_item_id);

SetSlotPlan plan_set_slot(const SlotContents& current, const std::string& want_item_id, int64_t want_count);
}
