#include <amity/protocol.hpp>

namespace amity {

std::optional<Envelope> parse_envelope(const std::string& text, std::string& error) {
    nlohmann::json j;
    try {
        j = nlohmann::json::parse(text);
    } catch (const nlohmann::json::exception&) {
        error = "invalid json";
        return std::nullopt;
    }

    if (!j.is_object()) {
        error = "envelope must be a json object";
        return std::nullopt;
    }
    if (!j.contains("id") || !j["id"].is_string() || j["id"].get<std::string>().empty()) {
        error = "missing or invalid id";
        return std::nullopt;
    }
    if (!j.contains("type") || !j["type"].is_string() || j["type"].get<std::string>().empty()) {
        error = "missing or invalid type";
        return std::nullopt;
    }
    if (j.contains("data") && !j["data"].is_object()) {
        error = "data must be an object";
        return std::nullopt;
    }

    Envelope e;
    e.id = j["id"].get<std::string>();
    e.type = j["type"].get<std::string>();
    e.data = j.contains("data") ? j["data"] : nlohmann::json::object();
    return e;
}

std::string serialize_envelope(const Envelope& e) {
    nlohmann::json j;
    j["id"] = e.id;
    j["type"] = e.type;
    j["data"] = e.data;
    return j.dump();
}

std::string make_reply(const std::string& id, const std::string& type, nlohmann::json data) {
    return serialize_envelope(Envelope{id, type, std::move(data)});
}

std::string make_error(const std::string& id, const std::string& code, const std::string& message) {
    nlohmann::json data = nlohmann::json::object();
    data["code"] = code;
    data["message"] = message;
    return make_reply(id, "error", std::move(data));
}

}
