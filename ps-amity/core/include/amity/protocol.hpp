#pragma once

#include <nlohmann/json.hpp>
#include <optional>
#include <string>

namespace amity {

inline constexpr int PROTOCOL_VERSION = 2;

struct Envelope {
    std::string id;
    std::string type;
    nlohmann::json data;
};

std::optional<Envelope> parse_envelope(const std::string& text, std::string& error);
std::string serialize_envelope(const Envelope& e);
std::string make_reply(const std::string& id, const std::string& type, nlohmann::json data);
std::string make_error(const std::string& id, const std::string& code, const std::string& message);

}
