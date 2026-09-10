#include <amity/capability_registry.hpp>

#include <utility>

namespace amity {

void CapabilityRegistry::set(const std::string& op, bool available, std::string reason) {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = ops_.find(op);
    if (it != ops_.end() && it->second.available == available && it->second.reason == reason) {
        return;
    }
    ops_[op] = Entry{available, std::move(reason)};
    ++version_;
}

nlohmann::json CapabilityRegistry::snapshot() const {
    std::lock_guard<std::mutex> lock(mutex_);
    nlohmann::json ops = nlohmann::json::object();
    for (const auto& [op, state] : ops_) {
        ops[op] = nlohmann::json{
            {"available", state.available},
            {"reason", state.available ? nlohmann::json(nullptr) : nlohmann::json(state.reason)},
        };
    }
    return nlohmann::json{{"version", version_}, {"ops", std::move(ops)}};
}

bool CapabilityRegistry::available(const std::string& op, std::string& reason_out) const {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = ops_.find(op);
    if (it == ops_.end()) {
        reason_out = "unknown capability";
        return false;
    }
    reason_out = it->second.reason;
    return it->second.available;
}

}
