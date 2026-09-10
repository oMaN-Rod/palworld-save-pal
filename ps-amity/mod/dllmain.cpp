#include "amity_mod.hpp"

#define AMITY_API __declspec(dllexport)
extern "C"
{
    AMITY_API RC::CppUserModBase* start_mod() { return new AmityMod(); }
    AMITY_API void uninstall_mod(RC::CppUserModBase* mod) { delete mod; }
}
