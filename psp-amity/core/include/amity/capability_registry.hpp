#pragma once

#include <cstdint>
#include <map>
#include <mutex>
#include <nlohmann/json.hpp>
#include <string>
#include <utility>

namespace amity {

class CapabilityRegistry {
public:
    void set(const std::string& op, bool available, std::string reason);
    nlohmann::json snapshot() const;
    bool available(const std::string& op, std::string& reason_out) const;

private:
    struct Entry {
        bool available = false;
        std::string reason;
    };

    mutable std::mutex mutex_;
    std::map<std::string, Entry> ops_;
    std::uint64_t version_ = 0;
};

}
