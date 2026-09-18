#pragma once

#include <nlohmann/json.hpp>

namespace amity_rt
{
bool update_resolution_report();
nlohmann::json resolution_report_json();
}
