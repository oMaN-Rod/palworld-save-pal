#pragma once

#include <cstddef>
#include <string>
#include <vector>

namespace amity_sig
{
struct ObservedParam
{
    std::wstring name;
    std::wstring type;
    int size{};
};

struct ParamSpec
{
    const wchar_t* name;
    const wchar_t* type;
    int size;
};

struct FunctionSpec
{
    const char* display;
    const wchar_t* path;
    const ParamSpec* params;
    std::size_t param_count;
};

struct OpSpec
{
    const char* op;
    const FunctionSpec* functions;
    std::size_t function_count;
};

const OpSpec* op_specs(std::size_t& count);
const OpSpec* find_op_spec(const std::string& op);

const OpSpec* read_op_specs(std::size_t& count);
const OpSpec* find_read_op_spec(const std::string& op);

bool params_match(const FunctionSpec& spec, const std::vector<ObservedParam>& observed, std::wstring& detail);
}
