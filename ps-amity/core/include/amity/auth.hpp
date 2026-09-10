#pragma once

#include <amity/protocol.hpp>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace amity {

bool fixed_time_equals(std::string_view a, std::string_view b);

class Session {
public:
    struct Output {
        std::vector<std::string> send;
        bool close = false;
        std::optional<Envelope> request;
    };

    Session(std::string expected_token, std::string nonce, nlohmann::json hello_info,
            int max_auth_attempts = 3);

    Output on_message(const std::string& text);
    bool ready() const;

private:
    enum class State { WaitHello, WaitAuth, Ready };

    Output reject(const Envelope& e, const std::string& code, const std::string& message, bool close);
    Output finish(Output out);

    std::string expected_token_;
    std::string nonce_;
    nlohmann::json hello_info_;
    int max_auth_attempts_;
    int auth_attempts_ = 0;
    State state_ = State::WaitHello;
    bool closed_ = false;
};

}
