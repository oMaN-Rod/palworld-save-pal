#pragma once

#include <filesystem>
#include <string>

namespace amity {

std::filesystem::path default_endpoint_dir();
bool write_endpoint_file(const std::filesystem::path& dir, int port, const std::string& token, std::string& error);
void remove_endpoint_file(const std::filesystem::path& dir);

}
