#include <amity/item_slot_plan.hpp>

#include <map>
#include <utility>

namespace
{
const nlohmann::json* find_container(const nlohmann::json& containers, const std::string& container_id)
{
    if (!containers.is_array() || container_id.empty())
    {
        return nullptr;
    }
    for (const auto& container : containers)
    {
        if (!container.is_object())
        {
            continue;
        }
        auto id = container.find("containerId");
        if (id != container.end() && id->is_string() && id->get<std::string>() == container_id)
        {
            return &container;
        }
    }
    return nullptr;
}

bool slot_index_of(const nlohmann::json& slot, int32_t& out)
{
    auto index = slot.find("slotIndex");
    if (index == slot.end() || !index->is_number_integer())
    {
        return false;
    }
    out = index->get<int32_t>();
    return true;
}

std::string item_id_of(const nlohmann::json& slot)
{
    auto id = slot.find("staticItemId");
    return (id != slot.end() && id->is_string()) ? id->get<std::string>() : std::string{};
}

bool count_of(const nlohmann::json& slot, int64_t& out)
{
    auto count = slot.find("count");
    if (count == slot.end() || !count->is_number_integer())
    {
        return false;
    }
    out = count->get<int64_t>();
    return true;
}
}

namespace amity
{
SlotContents read_slot(const nlohmann::json& containers, const std::string& container_id, int32_t slot_index)
{
    SlotContents contents{};
    const nlohmann::json* container = find_container(containers, container_id);
    if (!container)
    {
        return contents;
    }
    contents.container_found = true;

    auto slots = container->find("slots");
    if (slots == container->end() || !slots->is_array())
    {
        return contents;
    }
    for (const auto& slot : *slots)
    {
        int32_t index = 0;
        if (!slot.is_object() || !slot_index_of(slot, index) || index != slot_index)
        {
            continue;
        }
        contents.static_item_id = item_id_of(slot);
        if (!count_of(slot, contents.count))
        {
            contents.degraded = true;
        }
        return contents;
    }
    return contents;
}

std::vector<SlotGain> gained_slots(const nlohmann::json& before,
                                    const nlohmann::json& after,
                                    const std::string& static_item_id)
{
    std::vector<SlotGain> gains{};
    if (!after.is_array() || static_item_id.empty())
    {
        return gains;
    }

    std::map<std::pair<std::string, int32_t>, int64_t> held_before{};
    if (before.is_array())
    {
        for (const auto& container : before)
        {
            if (!container.is_object())
            {
                continue;
            }
            auto id = container.find("containerId");
            auto slots = container.find("slots");
            if (id == container.end() || !id->is_string() || slots == container.end() || !slots->is_array())
            {
                continue;
            }
            for (const auto& slot : *slots)
            {
                int32_t index = 0;
                int64_t count = 0;
                if (!slot.is_object() || !slot_index_of(slot, index) || item_id_of(slot) != static_item_id ||
                    !count_of(slot, count))
                {
                    continue;
                }
                held_before[{id->get<std::string>(), index}] = count;
            }
        }
    }

    for (const auto& container : after)
    {
        if (!container.is_object())
        {
            continue;
        }
        auto id = container.find("containerId");
        auto slots = container.find("slots");
        if (id == container.end() || !id->is_string() || slots == container.end() || !slots->is_array())
        {
            continue;
        }
        const std::string container_id = id->get<std::string>();
        for (const auto& slot : *slots)
        {
            int32_t index = 0;
            int64_t count = 0;
            if (!slot.is_object() || !slot_index_of(slot, index) || item_id_of(slot) != static_item_id ||
                !count_of(slot, count))
            {
                continue;
            }
            int64_t previous = 0;
            auto known = held_before.find({container_id, index});
            if (known != held_before.end())
            {
                previous = known->second;
            }
            if (count > previous)
            {
                gains.push_back(SlotGain{container_id, index, count - previous});
            }
        }
    }
    return gains;
}

SetSlotPlan plan_set_slot(const SlotContents& current, const std::string& want_item_id, int64_t want_count)
{
    SetSlotPlan plan{};
    const bool wants_nothing = want_item_id.empty() || want_count <= 0;
    const bool holds_nothing = current.static_item_id.empty();

    if (wants_nothing)
    {
        plan.dispose_count = holds_nothing ? 0 : current.count;
        return plan;
    }
    if (holds_nothing)
    {
        plan.grant_count = want_count;
        return plan;
    }
    if (current.static_item_id != want_item_id)
    {
        plan.dispose_count = current.count;
        plan.grant_count = want_count;
        return plan;
    }
    if (want_count > current.count)
    {
        plan.grant_count = want_count - current.count;
    }
    else if (want_count < current.count)
    {
        plan.dispose_count = current.count - want_count;
    }
    return plan;
}
}
