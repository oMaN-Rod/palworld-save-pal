#include <amity/config.hpp>

#include <algorithm>
#include <cctype>
#include <charconv>

namespace amity {

namespace {

std::string_view trim(std::string_view s) {
    auto is_space = [](unsigned char c) { return std::isspace(c) != 0; };
    while (!s.empty() && is_space(static_cast<unsigned char>(s.front()))) s.remove_prefix(1);
    while (!s.empty() && is_space(static_cast<unsigned char>(s.back()))) s.remove_suffix(1);
    return s;
}

std::string lower(std::string_view s) {
    std::string out(s);
    std::transform(out.begin(), out.end(), out.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return out;
}

bool parse_port(std::string_view text, int& out) {
    int value = 0;
    const char* begin = text.data();
    const char* end = begin + text.size();
    auto result = std::from_chars(begin, end, value);
    if (result.ec != std::errc{} || result.ptr != end) {
        return false;
    }
    out = value;
    return true;
}

bool is_ipv4_literal(std::string_view s) {
    int octets = 0;
    size_t i = 0;
    while (i <= s.size()) {
        size_t start = i;
        while (i < s.size() && std::isdigit(static_cast<unsigned char>(s[i]))) ++i;
        size_t len = i - start;
        if (len == 0 || len > 3) return false;
        int value = 0;
        if (!parse_port(s.substr(start, len), value) || value > 255) return false;
        ++octets;
        if (i == s.size()) break;
        if (s[i] != '.') return false;
        ++i;
    }
    return octets == 4;
}

}

BridgeConfig parse_ini(std::string_view text) {
    BridgeConfig cfg;
    bool in_bridge = false;
    size_t pos = 0;

    while (pos <= text.size()) {
        size_t eol = text.find('\n', pos);
        std::string_view raw = text.substr(pos, eol == std::string_view::npos ? std::string_view::npos : eol - pos);
        pos = (eol == std::string_view::npos) ? text.size() + 1 : eol + 1;

        std::string_view line = trim(raw);
        if (line.empty() || line.front() == ';' || line.front() == '#') continue;

        if (line.front() == '[') {
            size_t close = line.find(']');
            if (close == std::string_view::npos) continue;
            in_bridge = lower(trim(line.substr(1, close - 1))) == "bridge";
            continue;
        }

        if (!in_bridge) continue;

        size_t eq = line.find('=');
        if (eq == std::string_view::npos) continue;
        std::string key = lower(trim(line.substr(0, eq)));
        std::string_view value = trim(line.substr(eq + 1));

        if (key == "name") {
            if (!value.empty()) cfg.name = std::string(value);
        } else if (key == "token") {
            cfg.token = std::string(value);
        } else if (key == "bind") {
            if (!value.empty()) cfg.bind = std::string(value);
        } else if (key == "port") {
            int port = 0;
            if (parse_port(value, port)) cfg.port = port;
        }
    }

    return cfg;
}

void apply_env_overrides(BridgeConfig& cfg, const EnvLookup& lookup) {
    if (auto v = lookup("PSPAMITY_NAME"); v && !v->empty()) cfg.name = *v;
    if (auto v = lookup("PSPAMITY_TOKEN"); v) cfg.token = *v;
    if (auto v = lookup("PSPAMITY_BIND"); v && !v->empty()) cfg.bind = *v;
    if (auto v = lookup("PSPAMITY_PORT"); v) {
        int port = 0;
        if (parse_port(*v, port)) cfg.port = port;
    }
}

bool is_loopback_bind(const std::string& bind) {
    return bind == "127.0.0.1" || bind == "localhost";
}

bool validate_config(const BridgeConfig& cfg, std::string& error) {
    if (cfg.port != 0 && (cfg.port < 1024 || cfg.port > 65535)) {
        error = "port must be 0 or between 1024 and 65535";
        return false;
    }
    if (!is_loopback_bind(cfg.bind) && !is_ipv4_literal(cfg.bind)) {
        error = "bind must be 127.0.0.1, localhost, 0.0.0.0 or an IPv4 address";
        return false;
    }
    if (!is_loopback_bind(cfg.bind) && cfg.token.empty()) {
        error = "a non-loopback bind requires an explicit token";
        return false;
    }
    error.clear();
    return true;
}

}
