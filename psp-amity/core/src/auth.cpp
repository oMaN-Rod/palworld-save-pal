#include <amity/auth.hpp>

#include <amity/hmac.hpp>

namespace amity {

bool fixed_time_equals(std::string_view a, std::string_view b) {
    if (a.size() != b.size()) {
        return false;
    }
    unsigned char diff = 0;
    for (size_t i = 0; i < a.size(); ++i) {
        diff |= static_cast<unsigned char>(a[i]) ^ static_cast<unsigned char>(b[i]);
    }
    return diff == 0;
}

Session::Session(std::string expected_token, std::string nonce, nlohmann::json hello_info,
                 int max_auth_attempts)
    : expected_token_(std::move(expected_token)),
      nonce_(std::move(nonce)),
      hello_info_(std::move(hello_info)),
      max_auth_attempts_(max_auth_attempts) {}

bool Session::ready() const {
    return state_ == State::Ready;
}

Session::Output Session::reject(const Envelope& e, const std::string& code, const std::string& message, bool close) {
    Output out;
    out.send.push_back(make_error(e.id, code, message));
    out.close = close;
    return finish(std::move(out));
}

Session::Output Session::finish(Output out) {
    if (out.close) {
        closed_ = true;
    }
    return out;
}

Session::Output Session::on_message(const std::string& text) {
    if (closed_) {
        Output out;
        out.close = true;
        return out;
    }

    std::string err;
    auto parsed = parse_envelope(text, err);

    if (!parsed) {
        Output out;
        out.send.push_back(make_error("unknown", "bad_request", err));
        out.close = (state_ != State::Ready);
        return finish(std::move(out));
    }

    Envelope& e = *parsed;

    if (state_ == State::Ready) {
        Output out;
        out.request = std::move(e);
        return out;
    }

    if (state_ == State::WaitHello) {
        if (e.type == "hello") {
            int version = (e.data.contains("protocolVersion") && e.data["protocolVersion"].is_number_integer())
                ? e.data["protocolVersion"].get<int>()
                : -1;
            if (version != PROTOCOL_VERSION) {
                return reject(e, "protocol_mismatch", "unsupported protocol version", true);
            }
            nlohmann::json data = hello_info_;
            data["protocolVersion"] = PROTOCOL_VERSION;
            data["nonce"] = nonce_;
            Output out;
            out.send.push_back(make_reply(e.id, "hello_ok", std::move(data)));
            state_ = State::WaitAuth;
            return out;
        }
        if (e.type == "auth") {
            return reject(e, "bad_request", "auth before hello", true);
        }
        return reject(e, "unauthorized", "authentication required", true);
    }

    if (e.type == "auth") {
        std::string proof = (e.data.contains("proof") && e.data["proof"].is_string())
            ? e.data["proof"].get<std::string>()
            : std::string();
        if (!proof.empty() && fixed_time_equals(proof, hmac_sha256_hex(expected_token_, nonce_))) {
            state_ = State::Ready;
            Output out;
            out.send.push_back(make_reply(e.id, "auth_ok", nlohmann::json::object()));
            return out;
        }
        ++auth_attempts_;
        return reject(e, "unauthorized", "invalid proof", auth_attempts_ >= max_auth_attempts_);
    }
    if (e.type == "hello") {
        return reject(e, "bad_request", "hello already received", true);
    }
    return reject(e, "unauthorized", "authentication required", true);
}

}
