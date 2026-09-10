#include <amity/hmac.hpp>

#include <windows.h>
#include <bcrypt.h>

#include <array>
#include <stdexcept>
#include <vector>

namespace amity {

namespace {

struct AlgProvider {
    BCRYPT_ALG_HANDLE handle = nullptr;

    AlgProvider() {
        NTSTATUS status = BCryptOpenAlgorithmProvider(&handle, BCRYPT_SHA256_ALGORITHM, nullptr,
                                                      BCRYPT_ALG_HANDLE_HMAC_FLAG);
        if (!BCRYPT_SUCCESS(status)) {
            throw std::runtime_error("BCryptOpenAlgorithmProvider failed");
        }
    }

    ~AlgProvider() {
        if (handle) BCryptCloseAlgorithmProvider(handle, 0);
    }

    AlgProvider(const AlgProvider&) = delete;
    AlgProvider& operator=(const AlgProvider&) = delete;
};

}

std::string hmac_sha256_hex(std::string_view key, std::string_view message) {
    AlgProvider alg;

    BCRYPT_HASH_HANDLE hash = nullptr;
    NTSTATUS status = BCryptCreateHash(
        alg.handle, &hash, nullptr, 0,
        reinterpret_cast<PUCHAR>(const_cast<char*>(key.data())), static_cast<ULONG>(key.size()), 0);
    if (!BCRYPT_SUCCESS(status)) {
        throw std::runtime_error("BCryptCreateHash failed");
    }

    std::array<unsigned char, 32> digest{};
    status = BCryptHashData(hash, reinterpret_cast<PUCHAR>(const_cast<char*>(message.data())),
                            static_cast<ULONG>(message.size()), 0);
    if (BCRYPT_SUCCESS(status)) {
        status = BCryptFinishHash(hash, digest.data(), static_cast<ULONG>(digest.size()), 0);
    }
    BCryptDestroyHash(hash);
    if (!BCRYPT_SUCCESS(status)) {
        throw std::runtime_error("BCrypt HMAC computation failed");
    }

    static const char kHexDigits[] = "0123456789abcdef";
    std::string out;
    out.reserve(digest.size() * 2);
    for (unsigned char b : digest) {
        out.push_back(kHexDigits[b >> 4]);
        out.push_back(kHexDigits[b & 0x0F]);
    }
    return out;
}

}
