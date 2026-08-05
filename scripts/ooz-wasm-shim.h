#pragma once
// libc++ >= 18 (shipped with current emscripten/emsdk) removed the
// std::char_traits base template for non-character types. ooz's
// compr_multiarray.cpp instantiates std::basic_string<uint8>, so provide the
// missing specialization here. Force-included via `-include` in
// build-ooz-wasm.sh so the vendored sources stay unmodified.
#include <cstring>
#include <cwchar>
#include <iosfwd>
#include <string>

// Note: user specialization of std::char_traits is technically non-standard (libc++ removed this trait); WASM-build-only workaround.
template <>
struct std::char_traits<unsigned char> {
  using char_type = unsigned char;
  using int_type = int;
  using off_type = std::streamoff;
  using pos_type = std::streampos;
  using state_type = std::mbstate_t;

  static void assign(char_type &a, const char_type &b) noexcept { a = b; }
  static bool eq(char_type a, char_type b) noexcept { return a == b; }
  static bool lt(char_type a, char_type b) noexcept { return a < b; }
  static int compare(const char_type *a, const char_type *b, size_t n) {
    return n == 0 ? 0 : std::memcmp(a, b, n);
  }
  static size_t length(const char_type *s) {
    return std::strlen(reinterpret_cast<const char *>(s));
  }
  static const char_type *find(const char_type *s, size_t n, const char_type &c) {
    return static_cast<const char_type *>(std::memchr(s, c, n));
  }
  static char_type *move(char_type *dst, const char_type *src, size_t n) {
    if (n != 0) std::memmove(dst, src, n);
    return dst;
  }
  static char_type *copy(char_type *dst, const char_type *src, size_t n) {
    if (n != 0) std::memcpy(dst, src, n);
    return dst;
  }
  static char_type *assign(char_type *dst, size_t n, char_type c) {
    if (n != 0) std::memset(dst, c, n);
    return dst;
  }
  static int_type not_eof(int_type c) noexcept { return c != eof() ? c : 0; }
  static char_type to_char_type(int_type c) noexcept { return static_cast<char_type>(c); }
  static int_type to_int_type(char_type c) noexcept { return c; }
  static bool eq_int_type(int_type a, int_type b) noexcept { return a == b; }
  static int_type eof() noexcept { return -1; }
};
