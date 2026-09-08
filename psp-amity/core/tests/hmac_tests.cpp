#include <doctest/doctest.h>
#include <amity/hmac.hpp>

#include <string>

TEST_CASE("hmac_sha256_hex matches RFC 4231 test case 1") {
    std::string key(20, '\x0b');
    CHECK(amity::hmac_sha256_hex(key, "Hi There") ==
          "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
}

TEST_CASE("hmac_sha256_hex matches RFC 4231 test case 2") {
    CHECK(amity::hmac_sha256_hex("Jefe", "what do ya want for nothing?") ==
          "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
}

TEST_CASE("hmac_sha256_hex matches RFC 4231 test case 4") {
    std::string key;
    for (int i = 1; i <= 25; ++i) key.push_back(static_cast<char>(i));
    std::string message(50, '\xcd');
    CHECK(amity::hmac_sha256_hex(key, message) ==
          "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b");
}

TEST_CASE("hmac_sha256_hex is deterministic and nonce-sensitive") {
    auto a = amity::hmac_sha256_hex("token", "nonce-one");
    auto b = amity::hmac_sha256_hex("token", "nonce-one");
    auto c = amity::hmac_sha256_hex("token", "nonce-two");
    CHECK(a == b);
    CHECK(a != c);
    CHECK(a.size() == 64);
}

TEST_CASE("hmac_sha256_hex accepts an empty key and message") {
    CHECK(amity::hmac_sha256_hex("", "").size() == 64);
}
