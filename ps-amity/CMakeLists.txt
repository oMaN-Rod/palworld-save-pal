cmake_minimum_required(VERSION 3.22)
project(PSAmity CXX)

file(GLOB AMITY_IXWS_SOURCES third_party/IXWebSocket/ixwebsocket/*.cpp)
add_library(amity-ixws STATIC ${AMITY_IXWS_SOURCES})
target_include_directories(amity-ixws PUBLIC third_party/IXWebSocket)
target_compile_features(amity-ixws PRIVATE cxx_std_17)
target_compile_definitions(amity-ixws PRIVATE _WIN32_WINNT=0x0A00 _CRT_SECURE_NO_WARNINGS)
target_link_libraries(amity-ixws PUBLIC ws2_32 shlwapi crypt32)

file(GLOB AMITY_CORE_SOURCES core/src/*.cpp)
add_library(AmityCore STATIC ${AMITY_CORE_SOURCES})
target_include_directories(AmityCore PUBLIC core/include third_party)
target_compile_features(AmityCore PUBLIC cxx_std_20)
target_compile_definitions(AmityCore PRIVATE _WIN32_WINNT=0x0A00)
target_link_libraries(AmityCore PUBLIC amity-ixws bcrypt)

file(GLOB AMITY_TEST_SOURCES core/tests/*.cpp)
# mod/signature_check.cpp is Unreal-free by design, so the write-op signature table is covered
# by the core suite instead of only being exercised inside the game.
add_executable(AmityCoreTests ${AMITY_TEST_SOURCES} mod/signature_check.cpp)
target_include_directories(AmityCoreTests PRIVATE mod)
target_link_libraries(AmityCoreTests PRIVATE AmityCore)
target_compile_definitions(AmityCoreTests PRIVATE
    AMITY_FIXTURES_DIR="${CMAKE_CURRENT_SOURCE_DIR}/fixtures")

add_library(PSAmity SHARED
    mod/dllmain.cpp
    mod/amity_mod.cpp
    mod/reflect.cpp
    mod/resolution_report.cpp
    mod/signature_check.cpp
    mod/game_commands.cpp
    mod/game_call.cpp
    mod/command_item_set_slot.cpp
    mod/command_pal_slot.cpp
    mod/command_pal_edit.cpp
    mod/command_player_edit.cpp
    mod/game_executor.cpp
    mod/game_thread.cpp
    mod/snapshot_players.cpp
    mod/snapshot_pals.cpp
    mod/snapshot_inventory.cpp
    mod/snapshot_guild.cpp
    mod/snapshot_reflect.cpp
    mod/command_guild_edit.cpp
)
target_include_directories(PSAmity PRIVATE mod)
target_compile_features(PSAmity PRIVATE cxx_std_23)
target_compile_definitions(PSAmity PRIVATE NOMINMAX WIN32_LEAN_AND_MEAN)
target_link_libraries(PSAmity PUBLIC UE4SS)
target_link_libraries(PSAmity PRIVATE AmityCore)
set_target_properties(PSAmity PROPERTIES OUTPUT_NAME "PSAmity")
