#pragma once

#include <string>
#include <string_view>

namespace amity {

std::string hmac_sha256_hex(std::string_view key, std::string_view message);

}
