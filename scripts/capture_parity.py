#!/usr/bin/env python3
"""Capture WS request/response fixtures from the RUNNING Python backend.

The recorded fixtures are replayed against the Rust server by
psp-server/tests/parity.rs. Fixtures are machine-local (save_dir defaults,
loaded saves) and are gitignored — regenerate, don't commit.

Usage (backend must be running with a FRESH psp.db and NO save loaded for the
static-data corpus):
    uv run --with websockets scripts/capture_parity.py --corpus static-data
"""

import argparse
import asyncio
import io
import json
import os
import pathlib
import sys
import zipfile
from pathlib import Path
from typing import Optional

import websockets

# Every other scenario in this script talks to palworld_save_pal ONLY over
# the WS wire, against a separately-running backend process, so it never
# needs the package importable here. The gamepass scenario's prepare step
# (build_gamepass_corpus / prepare_gamepass_corpus below) is the first
# exception: it has to build the synthetic wgs container tree and write
# psp.db's settings row OUT OF BAND, before any backend process starts. When
# this file is run directly (`python scripts/capture_parity.py`), Python
# puts the SCRIPT's own directory on sys.path[0] (not the repo root), so the
# repo-root `palworld_save_pal/` package -- not pip-installed, only present
# as a source tree -- isn't importable without this.
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

# Requests for a fresh backend with no save loaded. One fixture per request.
# get_presets is EXCLUDED here — it has its own DB_PRESETS_SCENARIO below,
# since its dict-of-uuid-keyed response needs a dedicated capture flow (a
# fresh presets table) and a custom Rust-side comparator, not the plain
# masked strict-equality path every other static-data request uses.
STATIC_DATA_SCENARIO = [
    {"type": "get_version"},
    {"type": "get_settings"},
    {"type": "sync_app_state"},
    {"type": "get_active_skills"},
    {"type": "get_passive_skills"},
    {"type": "get_technologies"},
    {"type": "get_elements"},
    {"type": "get_items"},
    {"type": "get_missions"},
    {"type": "get_buildings"},
    {"type": "get_work_suitability"},
    {"type": "get_exp_data"},
    {"type": "get_friendship_data"},
    {"type": "get_map_objects"},
    {"type": "get_fast_travel_points"},
    {"type": "get_effigies"},
    {"type": "get_ui_common"},
    {"type": "get_pals"},
    {"type": "get_lab_research"},
]


def build_save_zip_bytes(save_dir: str) -> bytes:
    """Zip the corpus save the same way the UI would upload it:
    Level.sav, optional LevelMeta.sav, optional GlobalPalStorage.sav (Task
    3D-3's gps scenario -- see zip_gps_temp_path in
    psp-server/src/handlers/save_file.rs / save_file_handler.py:205 for
    the temp-file lazy-load path this exercises), Players/*.sav at the
    archive root."""
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.write(os.path.join(save_dir, "Level.sav"), "Level.sav")
        level_meta = os.path.join(save_dir, "LevelMeta.sav")
        if os.path.exists(level_meta):
            archive.write(level_meta, "LevelMeta.sav")
        global_pal_storage = os.path.join(save_dir, "GlobalPalStorage.sav")
        if os.path.exists(global_pal_storage):
            archive.write(global_pal_storage, "GlobalPalStorage.sav")
        players_dir = os.path.join(save_dir, "Players")
        for file_name in sorted(os.listdir(players_dir)):
            if file_name.endswith(".sav"):
                archive.write(
                    os.path.join(players_dir, file_name), f"Players/{file_name}"
                )
    return buffer.getvalue()


def load_path_requests(save_dir: str) -> list[dict]:
    """Ordered request sequence for the Phase 1 load-path fixtures:
    select_save (steam) -> sync_app_state -> load_zip_file."""
    level_sav_path = os.path.join(save_dir, "Level.sav")
    return [
        {
            "type": "select_save",
            "data": {"type": "steam", "path": level_sav_path, "local": False},
        },
        {"type": "sync_app_state", "data": None},
        {"type": "load_zip_file", "data": list(build_save_zip_bytes(save_dir))},
    ]


# Task 3B-3: exercises the presets CRUD surface (get/add/get/nuke/get) plus
# export_preset's pre-dialog validation error, against a FRESH presets table.
# get_presets responses are DICT-keyed by server-generated uuids (also
# embedded as each preset's own `id`/`pal_preset_id`), so parity.rs replays
# this corpus with a custom comparator (compare_get_presets_equivalent)
# instead of the usual masked strict-equality path — see
# parity/README.md, "db-presets scenario", for the safe capture
# procedure (this backend's psp.db must NOT be the developer's real one).
DB_PRESETS_SCENARIO = (
    "db-presets",
    [
        {"type": "get_presets", "data": None},
        {
            "type": "add_preset",
            "data": {
                "name": "Parity Kit",
                "type": "inventory",
                "skills": None,
                "common_container": [{"static_id": "Wood", "count": 999, "slot_index": 0}],
                "essential_container": None,
                "weapon_load_out_container": None,
                "player_equipment_armor_container": None,
                "food_equip_container": None,
                "storage_container": None,
                "pal_preset": None,
            },
        },
        {"type": "get_presets", "data": None},
        {"type": "nuke_presets", "data": None},
        {"type": "get_presets", "data": None},
        {
            "type": "export_preset",
            "data": {
                "preset_id": "does-not-exist",
                "preset_type": "inventory",
                "preset_name": "X",
            },
        },
    ],
)

# Task 3C-6: the UPS database surface (stats/collections/tags/pal CRUD),
# against a FRESH psp.db — mirrors the same 30-field pal DTO the Rust
# ups_ws.rs::sample_pal_dto test uses. This scenario exercises ONLY the
# DB-backed UPS handlers; clone_to_ups/import_to_ups/export_ups_pal need a
# loaded save and are covered by session-level tests, not this corpus.
#
# get_ups_pals returns an ARRAY of records each with independently-generated
# created_at/updated_at/last_accessed_at/instance_id, and add_ups_pal echoes
# the whole record — parity.rs masks those per-element/per-record fields (see
# mask_ups_list_frames + the add_ups_pal/... PARITY_IGNORED_PATHS entries).
# See parity/README.md, "db-ups scenario", for the safe capture procedure
# (this backend's psp.db must NOT be the developer's real one).
SAMPLE_PAL_DTO = {
    "instance_id": "11111111-1111-1111-1111-111111111111",
    "owner_uid": None,
    "character_id": "SheepBall",
    "is_lucky": False,
    "is_boss": False,
    "gender": "Female",
    "rank_hp": 1,
    "rank_attack": 2,
    "rank_defense": 3,
    "rank_craftspeed": 4,
    "talent_hp": 50,
    "talent_shot": 60,
    "talent_defense": 70,
    "rank": 1,
    "level": 12,
    "exp": 3450,
    "nickname": "Fluffy",
    "is_tower": False,
    "storage_id": "22222222-2222-2222-2222-222222222222",
    "stomach": 100.0,
    "storage_slot": 3,
    "learned_skills": [],
    "active_skills": [],
    "passive_skills": [],
    "hp": 5000,
    "max_hp": 5000,
    "group_id": None,
    "sanity": 100.0,
    "work_suitability": {"Handcraft": 1},
    "is_sick": False,
    "friendship_point": 0,
}

DB_UPS_SCENARIO = (
    "db-ups",
    [
        {"type": "get_ups_stats", "data": None},
        {"type": "get_ups_collections", "data": None},
        {
            "type": "create_ups_collection",
            "data": {"name": "Parity Favs", "description": "d", "color": "#f00"},
        },
        {"type": "get_ups_tags", "data": None},
        {
            "type": "create_ups_tag",
            "data": {"name": "parity-tag", "description": None, "color": "#0f0"},
        },
        {
            "type": "add_ups_pal",
            "data": {
                "pal_dto": SAMPLE_PAL_DTO,
                "source_save_file": "ParityWorld",
                "source_player_uid": "55555555-5555-5555-5555-555555555555",
                "source_player_name": "Omar",
                "source_storage_type": "pal_box",
                "source_storage_slot": 3,
                "collection_id": None,
                "tags": ["parity-tag"],
                "notes": None,
            },
        },
        {
            "type": "get_ups_pals",
            "data": {
                "offset": 0,
                "limit": 30,
                "search_query": None,
                "character_id_filter": None,
                "collection_id": None,
                "tags": None,
                "element_types": None,
                "pal_types": None,
                "sort_by": "created_at",
                "sort_order": "desc",
            },
        },
        {
            "type": "get_ups_all_filtered_ids",
            "data": {
                "search_query": None,
                "character_id_filter": None,
                "collection_id": None,
                "tags": ["parity-tag"],
                "element_types": None,
                "pal_types": None,
            },
        },
        {"type": "update_ups_pal", "data": {"pal_id": 1, "updates": {"nickname": "Rex"}}},
        {"type": "clone_ups_pal", "data": {"pal_id": 1}},
        {"type": "get_ups_stats", "data": None},
        {"type": "delete_ups_pals", "data": {"pal_ids": [1]}},
        {"type": "nuke_ups_pals", "data": None},
        {
            "type": "update_ups_collection",
            "data": {"collection_id": 1, "updates": {"is_favorite": True}},
        },
        {
            "type": "update_ups_tag",
            "data": {"tag_id": 1, "updates": {"name": "parity-tag-2"}},
        },
        {"type": "delete_ups_tag", "data": {"tag_id": 1}},
        {"type": "delete_ups_collection", "data": {"collection_id": 1}},
    ],
)

# Task 3E-5: the tools surface's remaining unexercised requests --
# convert_steam_id's non-uid input shapes (a vanity URL, garbage input, and a
# non-standard-length hex string), swap_player_uids's no-save error path, and
# get_raw_data's no-save/no-target-set empty-object path. No save is loaded
# (matches convert_steam_id/swap_player_uids' own no-save-needed nature), so
# this needs no --save-dir and, unlike db-presets/db-ups, touches no psp.db
# table at all (same as the transfer scenario) -- back the DB up anyway as
# defensive practice; see parity/README.md, "tools scenario".
#
# get_raw_data's captured `data` is compared STRUCTURALLY, not value-exact,
# by parity.rs's PARITY_STRUCTURAL_TYPES (Contract deviation 6: Rust's
# get_raw_data echoes uesave's own JSON dialect, Python's echoes
# palworld-save-tools' GVAS-dict form -- two different, non-comparable JSON
# shapes for the SAME underlying save data). With no save loaded here, both
# backends answer `{}` for it -- a live-save fixture that exercises a
# resolved (non-empty) target belongs in a future save-backed corpus, not
# this one.
TOOLS_SCENARIO = (
    "tools",
    [
        {"type": "convert_steam_id", "data": {"steam_input": "76561198000000001"}},
        {
            "type": "convert_steam_id",
            "data": {"steam_input": "https://steamcommunity.com/id/vanity"},
        },
        {"type": "convert_steam_id", "data": {"steam_input": "garbage!!"}},
        {
            "type": "convert_steam_id",
            "data": {"steam_input": "AABBCCDD000000000000000000000000"},
        },
        {
            "type": "swap_player_uids",
            "data": {
                "old_player_uid": "55555555-5555-5555-5555-555555555555",
                "new_player_uid": "55555555-5555-5555-5555-555555555555",
            },
        },
        {
            "type": "get_raw_data",
            "data": {
                "guild_id": None,
                "player_id": None,
                "pal_id": None,
                "base_id": None,
                "item_container_id": None,
                "character_container_id": None,
                "level": False,
            },
        },
    ],
)

# Task P6-14: the servers scenario -- exercises the deterministic subset of
# the server-management surface (list/get/get_stats/toggle_mod against an
# UNKNOWN server id, plus detect_workshop_dir). Server management is
# otherwise host-environment-dependent (Docker daemon, steamcmd, live
# processes), so this corpus deliberately covers only what's fully
# deterministic against a FRESH, EMPTY psp.db with no server rows:
# list_servers answers an empty list, and every id-424242 request answers
# "Server not found" (server 424242 is never created here). detect_workshop_dir
# is the one exception -- its own response IS machine-dependent (it echoes
# whatever Steam workshop install path THIS capture machine resolves, or ""
# if none is found) -- record it as-is; psp-server/tests/parity.rs masks
# "/data/workshop_dir" for that response type (PARITY_IGNORED_PATHS,
# "machine-dependent Steam install location") so replay doesn't require the
# replay machine to have the same Steam install.
SERVERS_SCENARIO = (
    "servers",
    [
        {"type": "list_servers", "data": None},
        {"type": "get_server", "data": {"server_id": 424242}},
        {"type": "get_server_stats", "data": {"server_id": 424242}},
        {
            "type": "toggle_server_mod",
            "data": {"server_id": 424242, "mod_name": "X", "enabled": True},
        },
        {"type": "detect_workshop_dir", "data": None},
    ],
)

# ---------------------------------------------------------------------------
# Task P4-14: the gamepass scenario.
#
# Exercises the gamepass load/convert/save-back/unlock-map surface:
# `select_save` (gamepass branch) -> `select_gamepass_save` ->
# `convert_save_format` (standalone gamepass->steam) -> `save_modded_save`
# (gamepass branch) -> `unlock_map`. Unlike load_path/phase2/gps/transfer,
# this scenario's INPUT isn't a pre-existing corpus save directory -- it's a
# synthetic wgs container tree that has to be BUILT before the Python
# backend even starts (build_gamepass_corpus), because the backend's
# Settings object caches `save_dir` at import time (see "Known Python quirks
# affecting capture" -- the same import-order fact, applied here on purpose
# rather than worked around). So the gamepass scenario needs a PREPARE step
# run standalone, before the backend starts:
#
#   uv run --with websockets scripts/capture_parity.py \
#       --scenario gamepass --prepare-gamepass \
#       --save-dir "<absolute path to tests/fixtures/saves/world2>"
#
# which builds the container tree under parity/tmp/gamepass/wgs/...,
# writes its absolute path into the (freshly-created) psp.db's settings row,
# and prints the container dir for the next step. See parity/README.md,
# "gamepass scenario", for the full safe-capture procedure (fresh psp.db,
# same discipline as db-presets/db-ups/gps/transfer/tools).
GAMEPASS_TMP = Path(__file__).resolve().parents[1] / "parity" / "tmp" / "gamepass"
GAMEPASS_SAVE_ID = "0123456789ABCDEF0123456789ABCDEF"
# world2 (exactly 1 player) -- NOT world1 (2 players) -- for the same reason
# the load_path scenario's README section documents: Python's
# _extract_players_parallel races on a ThreadPoolExecutor above 1 player,
# making player-summary array order genuinely nondeterministic run-to-run.


def _graft_world_map_mask(level_meta_bytes: bytes, mask: bytes) -> bytes:
    """Return `level_meta_bytes` re-emitted with a synthetic
    `SaveData.WorldMapMaskTextureV4` byte-array property grafted in. Neither
    `world1` nor `world2` (this repo's only steam corpora) has a real
    LocalData.sav, so `unlock_map` (request 05) has nothing to unlock unless
    one is synthesized. This mirrors, in Python (for capture authoritativeness),
    the EXACT technique psp-core/src/localdata.rs's own
    `unlock_world_map_zeroes_synthetic_mask_grafted_into_real_savedata` test
    uses on the Rust side: graft the mask into a COPY of the corpus
    LevelMeta.sav's `SaveData` struct (unlock_map only ever reads
    `SaveData.WorldMapMaskTextureV4`; it never validates the rest of the
    struct's shape, so a LevelMeta-shaped carrier is fine) and re-emit as a
    `LocalData.sav`-shaped PlM/Oodle payload. Property dict shape verified
    empirically against palworld_save_tools' archive.py (`_write_ArrayProperty`
    -> `array_property` -> the `ByteProperty` fast path, which accepts a
    plain `bytes` value)."""
    from palworld_save_tools.gvas import GvasFile
    from palworld_save_tools.palsav import compress_gvas_to_sav, decompress_sav_to_gvas
    from palworld_save_tools.paltypes import PALWORLD_CUSTOM_PROPERTIES, PALWORLD_TYPE_HINTS

    raw_gvas, _ = decompress_sav_to_gvas(level_meta_bytes)
    gvas_file = GvasFile.read(
        raw_gvas, PALWORLD_TYPE_HINTS, PALWORLD_CUSTOM_PROPERTIES, allow_nan=True
    )
    save_data = gvas_file.properties["SaveData"]["value"]
    save_data["WorldMapMaskTextureV4"] = {
        "array_type": "ByteProperty",
        "id": None,
        "value": {"values": mask},
        "type": "ArrayProperty",
    }
    return compress_gvas_to_sav(gvas_file.write(PALWORLD_CUSTOM_PROPERTIES), 0x31)


def build_gamepass_corpus(steam_save_dir: Path) -> Path:
    """Package the primary steam corpus save into a synthetic wgs container
    dir, snapshot it (`wgs-pristine`, for the Rust replay's pre-fixture
    reset), and stage a synthetic `LocalData.sav` (+ an untouched
    `LocalData.sav.pristine` copy, since `unlock_map` mutates its input file
    IN PLACE -- map_unlock_handler.py:82-83 / save_file.rs's
    `unlock_map_on_disk` both overwrite `path` with the zeroed bytes) for the
    `unlock_map` request."""
    import shutil
    from datetime import datetime

    from palworld_save_pal.utils.gamepass.container_types import FILETIME, ContainerIndex
    from palworld_save_pal.utils.gamepass.container_utils import create_new_container

    container_dir = (
        GAMEPASS_TMP / "wgs" / "0009000000000000_00000000000000000000000000000000"
    )
    if GAMEPASS_TMP.exists():
        shutil.rmtree(GAMEPASS_TMP)
    container_dir.mkdir(parents=True)

    index = ContainerIndex(
        flag1=0,
        package_name="PocketpairInc.Palworld_ad4psfrxyesvt",
        mtime=FILETIME.from_timestamp(datetime.now().timestamp()),
        flag2=0,
        index_uuid="",
        unknown=0,
        containers=[],
    )
    index.containers.append(
        create_new_container(
            str(container_dir), GAMEPASS_SAVE_ID, (steam_save_dir / "Level.sav").read_bytes()
        )
    )
    level_meta_bytes = (steam_save_dir / "LevelMeta.sav").read_bytes()
    index.containers.append(
        create_new_container(
            str(container_dir),
            GAMEPASS_SAVE_ID,
            level_meta_bytes,
            container_suffix="LevelMeta",
        )
    )
    for player_file in sorted((steam_save_dir / "Players").glob("*.sav")):
        index.containers.append(
            create_new_container(
                str(container_dir),
                GAMEPASS_SAVE_ID,
                player_file.read_bytes(),
                container_suffix=f"Players-{player_file.stem}",
            )
        )
    index.write_file(str(container_dir))

    shutil.copytree(container_dir, GAMEPASS_TMP / "wgs-pristine")

    local_data_bytes = _graft_world_map_mask(level_meta_bytes, bytes([1, 2, 3, 0, 4]))
    (GAMEPASS_TMP / "LocalData.sav").write_bytes(local_data_bytes)
    (GAMEPASS_TMP / "LocalData.sav.pristine").write_bytes(local_data_bytes)

    return container_dir


def prepare_gamepass_corpus(steam_save_dir: Path) -> Path:
    """The pre-backend-start half of the gamepass scenario: build the
    container tree, then persist its path into the (possibly brand-new)
    psp.db's settings row -- `create_db_and_tables()` runs the same idempotent
    `SQLModel.metadata.create_all` the real backend runs at startup, so this
    works against either a fresh or an already-warmed psp.db without the
    two-start dance the static-data corpus needs. Must run and complete
    BEFORE the Python backend process starts (see the module comment above)."""
    from palworld_save_pal.db.bootstrap import create_db_and_tables
    from palworld_save_pal.db.ctx.settings import update_save_dir

    container_dir = build_gamepass_corpus(steam_save_dir)
    create_db_and_tables()
    update_save_dir(str(container_dir))
    return container_dir


def gamepass_requests(container_dir: str) -> list[dict]:
    """The gamepass request sequence. `container_dir` here is NOT a
    Level.sav's parent (unlike every other SAVE_DIR_SCENARIOS entry) -- it is
    the wgs container directory `prepare_gamepass_corpus` printed, passed back
    in via `--save-dir` for this second, backend-running invocation.
    `_resolve_requests` hands every SAVE_DIR_SCENARIOS entry the raw
    `--save-dir` STRING (see `load_path_requests`'s own `str` parameter) --
    wrap it in `Path` here since this function needs `/`-joining."""
    container_dir = Path(container_dir)
    return [
        # 000: gamepass branch of select_save -> a select_gamepass_save
        # response (not select_save -- see local_file_handler.py:172-175).
        {
            "type": "select_save",
            "data": {
                "type": "gamepass",
                "path": str(container_dir / "containers.index"),
                "local": False,
            },
        },
        # 001: full load -> progress* / loaded_save_files / summaries.
        {"type": "select_gamepass_save", "data": GAMEPASS_SAVE_ID},
        # 002: standalone extraction BEFORE any mutation adds a second save
        # id -- _standalone_gamepass_to_steam iterates a SET of save ids
        # (convert_handler.py:502), whose order is only deterministic with
        # exactly one save present in the container index.
        {
            "type": "convert_save_format",
            "data": {
                "target_format": "steam",
                "source_path": str(container_dir),
                "output_path": str(GAMEPASS_TMP / "steam-out"),
            },
        },
        # 003: gamepass modded save (adds a new save id + a
        # wall-clock-timestamped backup dir).
        {"type": "save_modded_save", "data": "Parity Modded"},
        # 004: unlock_map on the staged synthetic LocalData.sav.
        {"type": "unlock_map", "data": {"path": str(GAMEPASS_TMP / "LocalData.sav")}},
    ]


# Scenarios with a fixed request list, independent of any on-disk save.
FIXED_SCENARIOS = {
    "static-data": STATIC_DATA_SCENARIO,
    DB_PRESETS_SCENARIO[0]: DB_PRESETS_SCENARIO[1],
    DB_UPS_SCENARIO[0]: DB_UPS_SCENARIO[1],
    TOOLS_SCENARIO[0]: TOOLS_SCENARIO[1],
    SERVERS_SCENARIO[0]: SERVERS_SCENARIO[1],
}

# Scenarios that build their request list from a corpus save directory
# (--save-dir), keyed by scenario name.
SAVE_DIR_SCENARIOS = {"load_path": load_path_requests, "gamepass": gamepass_requests}

# The Phase-2 dynamic scenario (see capture_phase2): it derives each request
# from earlier live responses, so it has no static request list and is handled
# by its own code path. Needs --save-dir like the SAVE_DIR_SCENARIOS.
PHASE2_SCENARIO = "phase2"

# A request's response burst is considered complete after this much silence.
IDLE_SECONDS = 2.0


def _null_save_dir_response_type(responses: list) -> Optional[str]:
    """Return the wire `type` of the first response carrying `data.save_dir:
    null`, or None if none do. Narrow, cheap scan for one known-bad capture
    symptom (see 'Known Python quirks affecting capture' in
    parity/README.md) — not a general fixture validator."""
    for response in responses:
        if not isinstance(response, dict):
            continue
        data = response.get("data")
        if isinstance(data, dict) and "save_dir" in data and data["save_dir"] is None:
            return response.get("type", "<unknown>")
    return None


def _resolve_requests(scenario: str, save_dir: Optional[str]) -> list[dict]:
    """Build the ordered request list for `scenario`. `FIXED_SCENARIOS`
    entries are used as-is; `SAVE_DIR_SCENARIOS` entries need `--save-dir`
    (a corpus save's directory, i.e. Level.sav's parent) to build requests
    that reference real on-disk paths."""
    if scenario in FIXED_SCENARIOS:
        return FIXED_SCENARIOS[scenario]
    if save_dir is None:
        print(
            f"error: scenario {scenario!r} requires --save-dir (the corpus "
            "save's directory, i.e. Level.sav's parent)",
            file=sys.stderr,
        )
        sys.exit(1)
    return SAVE_DIR_SCENARIOS[scenario](save_dir)


async def _drain_response_burst(socket) -> list:
    """Read frames until `IDLE_SECONDS` of silence marks the end of the
    response burst for the request just sent. Returns the parsed frames."""
    responses = []
    while True:
        try:
            frame = await asyncio.wait_for(socket.recv(), timeout=IDLE_SECONDS)
        except asyncio.TimeoutError:
            break
        responses.append(json.loads(frame))
    return responses


def _refuse_null_save_dir(request: dict, responses: list) -> None:
    """Abort (non-zero) if any response carries `save_dir: null` — a known
    unwarmed-psp.db capture artifact, NOT a real divergence."""
    offending_type = _null_save_dir_response_type(responses)
    if offending_type is None:
        return
    # A truly fresh psp.db makes Python's settings loader hit a missing table,
    # swallow the error, and report save_dir: null forever for that process's
    # life. This is a known, 100%-reproducible capture-time artifact, not a
    # legitimate Rust/Python divergence — recording it would be a trap for
    # whoever replays this fixture next. Refuse to write it; see
    # parity/README.md, "Known Python quirks affecting capture", for the
    # fix (warm the DB first) and why this must NOT become a
    # PARITY_IGNORED_PATHS mask.
    print(
        f"error: response type {offending_type!r} (for request "
        f"{request['type']!r}) has save_dir: null — the Python backend's "
        "settings table did not exist when it started. See 'Known Python "
        "quirks affecting capture' in parity/README.md: stop the backend, "
        "then start it again (a warmed psp.db) before capturing. Refusing to "
        "write this fixture.",
        file=sys.stderr,
    )
    sys.exit(1)


def _write_fixture(
    corpus_dir: pathlib.Path, index: int, request: dict, responses: list
) -> None:
    fixture_path = corpus_dir / f"{index:03d}_{request['type']}.json"
    fixture_path.write_text(
        json.dumps(
            {"request": request, "responses": responses},
            indent=2,
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )
    print(f"wrote {fixture_path} ({len(responses)} responses)")


def _find_response(responses: list, message_type: str) -> Optional[dict]:
    """First response frame whose wire `type` equals `message_type`, or None."""
    for response in responses:
        if isinstance(response, dict) and response.get("type") == message_type:
            return response
    return None


async def capture_corpus(
    url: str,
    scenario: str,
    corpus: str,
    save_dir: Optional[str],
    output_root: pathlib.Path,
) -> None:
    requests = _resolve_requests(scenario, save_dir)
    corpus_dir = output_root / corpus
    corpus_dir.mkdir(parents=True, exist_ok=True)
    try:
        socket_context = websockets.connect(url, max_size=2**30)
        async with socket_context as socket:
            for request_index, request in enumerate(requests):
                await socket.send(json.dumps(request))
                responses = await _drain_response_burst(socket)
                _refuse_null_save_dir(request, responses)
                _write_fixture(corpus_dir, request_index, request, responses)
    except (OSError, websockets.exceptions.InvalidHandshake) as connect_error:
        # websockets raises a plain OSError (e.g. ConnectionRefusedError) when
        # nothing is listening at --url, and InvalidHandshake/InvalidStatus
        # (a subclass) when something IS listening but rejects the WS
        # handshake (e.g. a non-numeric client_id path segment — see the
        # --url comment below). Fail loudly instead of letting either surface
        # as a raw traceback, or — worse — silently writing zero fixtures and
        # exiting 0.
        print(
            f"error: could not connect to {url}: {connect_error}\n"
            "Is the Python backend running? (uv run python psp.py --port 5174)",
            file=sys.stderr,
        )
        sys.exit(1)


# ---------------------------------------------------------------------------
# Phase-2 dynamic scenario.
#
# Unlike the fixed/save-dir scenarios above, Phase 2 needs ids that only exist
# in the LIVE responses (a player id, a guild id, an editable pal's instance
# id, and the player's container ids). It therefore builds each request from
# the responses of earlier ones, all on a SINGLE connection so the fixture
# corpus replays deterministically over one Rust WebSocket.
#
# ⚠️ The backend MUST be FRESH (no save/player pre-loaded in Python's GLOBAL
# app_state) when this runs: request_player_details is a lazy first-load whose
# `progress_message` frames must match Rust's per-connection first-load. If a
# previous connection already loaded this save/player into Python's global
# state, those progress frames vanish and replay diverges. Start a clean
# backend just before capturing.
# ---------------------------------------------------------------------------


async def capture_phase2(
    url: str,
    save_dir: str,
    corpus: str,
    output_root: pathlib.Path,
) -> None:
    corpus_dir = output_root / corpus
    corpus_dir.mkdir(parents=True, exist_ok=True)
    level_sav_path = os.path.join(save_dir, "Level.sav")

    index = 0
    try:
        socket_context = websockets.connect(url, max_size=2**30)
        async with socket_context as socket:

            async def run(request: dict) -> list:
                nonlocal index
                await socket.send(json.dumps(request))
                responses = await _drain_response_burst(socket)
                _refuse_null_save_dir(request, responses)
                _write_fixture(corpus_dir, index, request, responses)
                index += 1
                return responses

            # 000 select_save — load the corpus save; harvest the player/guild
            # summaries to pick a (preferably non-admin) player and a guild.
            select_responses = await run(
                {
                    "type": "select_save",
                    "data": {"type": "steam", "path": level_sav_path, "local": False},
                }
            )
            player_summaries = _find_response(select_responses, "get_player_summaries")
            guild_summaries = _find_response(select_responses, "get_guild_summaries")
            if player_summaries is None or guild_summaries is None:
                print(
                    "error: select_save did not return player/guild summaries — "
                    "cannot drive the Phase-2 sequence.",
                    file=sys.stderr,
                )
                sys.exit(1)
            player_ids = list(player_summaries["data"].keys())
            guilds = guild_summaries["data"]
            if not player_ids or not guilds:
                print(
                    "error: corpus has no players or no guilds; Phase-2 needs both.",
                    file=sys.stderr,
                )
                sys.exit(1)
            guild_id = next(iter(guilds))
            admin_uid = guilds[guild_id].get("admin_player_uid")
            # Prefer a player who is NOT this guild's admin, so delete_player
            # (a non-admin) then delete_guild stays a clean, ordered teardown.
            probe_order = [pid for pid in player_ids if pid != admin_uid] + [
                pid for pid in player_ids if pid == admin_uid
            ]

            # 001/002 static get_pals / get_pal_summaries.
            await run({"type": "get_pals", "data": None})
            await run({"type": "get_pal_summaries", "data": None})

            # Probe players (non-admin first) via request_player_details until
            # one carries an editable pal. Every probe is a legitimate fixture
            # (deterministic on both backends); the first with a pal is chosen.
            chosen = None
            for candidate in probe_order:
                details = await run(
                    {
                        "type": "request_player_details",
                        "data": {"player_id": candidate, "origin": "edit"},
                    }
                )
                response = _find_response(details, "get_player_details_response")
                player = response["data"].get("player") if response else None
                pals = player.get("pals") if isinstance(player, dict) else None
                if pals:
                    pal_id = next(iter(pals))
                    chosen = {
                        "player_id": candidate,
                        "pal_id": pal_id,
                        "pal_payload": pals[pal_id],
                        "otomo_container_id": player.get("otomo_container_id"),
                        "pal_box_id": player.get("pal_box_id"),
                        "is_admin": candidate == admin_uid,
                    }
                    break
            if chosen is None:
                print(
                    "error: no corpus player carries an editable pal — the "
                    "Phase-2 sequence needs one to edit/heal/move.",
                    file=sys.stderr,
                )
                sys.exit(1)

            player_id = chosen["player_id"]
            pal_id = chosen["pal_id"]
            pal_payload = chosen["pal_payload"]
            move_container_id = chosen["otomo_container_id"]
            add_container_id = chosen["pal_box_id"]
            edited_pal = {**pal_payload, "nickname": "ParityEdited", "level": 33}
            print(
                f"phase2: player_id={player_id} (admin={chosen['is_admin']}) "
                f"guild_id={guild_id} pal_id={pal_id} "
                f"move->{move_container_id} add->{add_container_id}"
            )

            # request_guild_details takes a BARE uuid string (ws/messages.py:755).
            await run({"type": "request_guild_details", "data": guild_id})
            await run({"type": "get_lab_research", "data": None})
            # heal_pals data is a bare uuid list; no response frame.
            await run({"type": "heal_pals", "data": [pal_id]})
            await run({"type": "heal_all_pals", "data": {"player_id": player_id}})
            # set_technology_data uses camelCase field names (ws/messages.py:436).
            await run(
                {
                    "type": "set_technology_data",
                    "data": {
                        "playerID": player_id,
                        "technologies": ["Workbench", "HandTorch"],
                        "techPoints": 42,
                        "ancientTechPoints": 7,
                    },
                }
            )
            # update_lab_research does a FULL replacement of research_info, so
            # any research_id succeeds deterministically on both backends.
            await run(
                {
                    "type": "update_lab_research",
                    "data": {
                        "guild_id": guild_id,
                        "research_updates": [
                            {"research_id": "Research_Tech_01", "work_amount": 250.0}
                        ],
                    },
                }
            )
            # Edit an EXISTING pal (byte-valid resave), then re-read it.
            await run(
                {
                    "type": "update_save_file",
                    "data": {"modified_pals": {pal_id: edited_pal}},
                }
            )
            await run(
                {
                    "type": "request_player_details",
                    "data": {"player_id": player_id, "origin": "edit"},
                }
            )
            await run(
                {
                    "type": "move_pal",
                    "data": {
                        "player_id": player_id,
                        "pal_id": pal_id,
                        "container_id": move_container_id,
                    },
                }
            )
            await run({"type": "rename_world", "data": "Parity World"})
            # download_save_file is captured BEFORE add_pal on purpose: add_pal
            # injects a fresh uuid4 InstanceId that Python and Rust generate
            # independently, which would diverge the Level.sav GVAS bytes and
            # defeat the deep byte-identity check the replay runs on the zip's
            # decompressed contents. Downloading first captures the maximal
            # DETERMINISTIC edited state (all heals/tech/lab/pal-edit/move/
            # rename applied); add_pal's own masked instance_id is exercised by
            # its own add_pal frame right after.
            await run({"type": "download_save_file", "data": None})
            await run(
                {
                    "type": "add_pal",
                    "data": {
                        "player_id": player_id,
                        "character_id": "SheepBall",
                        "nickname": "parity",
                        "container_id": add_container_id,
                    },
                }
            )
            # Deletes LAST: a non-admin player, then the guild.
            await run(
                {"type": "delete_player", "data": {"player_id": player_id, "origin": "edit"}}
            )
            await run(
                {"type": "delete_guild", "data": {"guild_id": guild_id, "origin": "edit"}}
            )
    except (OSError, websockets.exceptions.InvalidHandshake) as connect_error:
        print(
            f"error: could not connect to {url}: {connect_error}\n"
            "Is the Python backend running? (uv run python psp.py --port 5174)",
            file=sys.stderr,
        )
        sys.exit(1)

    print(f"phase2 capture complete: {index} fixtures written to {corpus_dir}")


# ---------------------------------------------------------------------------
# Task 3D-3: the GPS (Global Pal Storage) scenario.
#
# Dynamic like `phase2` (capture_phase2 above), not a FIXED_SCENARIOS list,
# for two reasons: (1) the zip upload has to be built from --save-dir at
# capture time via build_save_zip_bytes, which now also embeds
# GlobalPalStorage.sav when the corpus save dir has one; (2)
# clone_gps_pal_to_player's destination_player_uid needs a REAL player uid
# from THIS corpus (CORPUS_PLAYER_UID), harvested from load_zip_file's own
# get_player_summaries response burst -- there's no way to know it ahead of
# time from a static request list, the same reason phase2 harvests its
# player/guild/pal ids from live responses instead of hardcoding them.
#
# The sequence starts with `load_zip_file` directly -- no `select_save` /
# `sync_app_state` first, unlike `load_path_requests` -- so both backends
# exercise the ZIP-upload temp-file lazy-load path GPS reads
# GlobalPalStorage.sav from (`zip_gps_temp_path` in
# psp-server/src/handlers/save_file.rs; `save_file_handler.py:205` on
# the Python side), not the on-disk save_dir path `select_save` would use.
# `save_file.rs::handle_load_zip_file` still emits `get_player_summaries` /
# `get_guild_summaries` (shared with `select_save`/`sync_app_state`), so the
# player-uid harvest below works from this single request's response burst.
#
# NO GPS-containing corpus exists in this checkout (no
# `GlobalPalStorage.sav` alongside any `Level.sav`/`Players/` directory) --
# this function is scaffolding for a developer who has one to capture
# against later. See parity/README.md, "gps scenario", for the safe
# capture procedure.
GPS_SCENARIO = "gps"


async def capture_gps(
    url: str,
    save_dir: str,
    corpus: str,
    output_root: pathlib.Path,
) -> None:
    corpus_dir = output_root / corpus
    corpus_dir.mkdir(parents=True, exist_ok=True)

    index = 0
    try:
        socket_context = websockets.connect(url, max_size=2**30)
        async with socket_context as socket:

            async def run(request: dict) -> list:
                nonlocal index
                await socket.send(json.dumps(request))
                responses = await _drain_response_burst(socket)
                _refuse_null_save_dir(request, responses)
                _write_fixture(corpus_dir, index, request, responses)
                index += 1
                return responses

            # 000 load_zip_file -- zip upload (see module comment above for
            # why this, not select_save). Harvest a real player uid from the
            # get_player_summaries frame in this same response burst; that is
            # the CORPUS_PLAYER_UID substitution the clone_gps_pal_to_player
            # step below needs.
            load_responses = await run(
                {"type": "load_zip_file", "data": list(build_save_zip_bytes(save_dir))}
            )
            player_summaries = _find_response(load_responses, "get_player_summaries")
            player_ids = (
                list(player_summaries["data"].keys()) if player_summaries else []
            )
            if not player_ids:
                print(
                    "error: corpus has no players -- the gps scenario's "
                    "clone_gps_pal_to_player step needs a real "
                    "destination_player_uid (CORPUS_PLAYER_UID).",
                    file=sys.stderr,
                )
                sys.exit(1)
            corpus_player_uid = player_ids[0]
            print(f"gps: corpus_player_uid={corpus_player_uid}")

            # 001 request_gps -- first call lazy-loads GlobalPalStorage.sav
            # from the temp file staged by load_zip_file, so it emits a
            # progress_message before the get_gps_response frame.
            await run({"type": "request_gps", "data": None})
            # 002 request_gps -- second call returns the now-cached map, with
            # NO progress_message frame.
            await run({"type": "request_gps", "data": None})

            # 003 add_gps_pal -- mints a fresh uuid4 instance_id (masked by
            # PARITY_IGNORED_PATHS' "add_gps_pal:/data/pal/instance_id").
            await run(
                {
                    "type": "add_gps_pal",
                    "data": {
                        "character_id": "SheepBall",
                        "nickname": "ParitySheep",
                        "storage_slot": None,
                    },
                }
            )

            # 004 delete_gps_pals -- never responds (gps_handler.py:106-114 /
            # handlers/gps.rs::handle_delete_gps_pals), so this fixture
            # records zero response frames in both capture and replay.
            await run({"type": "delete_gps_pals", "data": {"pal_indexes": [0]}})

            # 005 request_gps -- re-read after the add/delete above; the
            # slot-keyed map's per-pal instance_id is masked by parity.rs's
            # get_gps_response walker (mask_gps_response_frame), not a static
            # PARITY_IGNORED_PATHS pointer -- see parity/README.md.
            await run({"type": "request_gps", "data": None})

            # 006 clone_gps_pal_to_player -- a pal id NOT present in GPS, so
            # this exercises the per-pal `errors` list path (no new
            # instance_id minted on this path, so nothing here needs
            # masking).
            await run(
                {
                    "type": "clone_gps_pal_to_player",
                    "data": {
                        "pal_ids": ["00000000-0000-0000-0000-00000000dead"],
                        "destination_type": "pal_box",
                        "destination_player_uid": corpus_player_uid,
                    },
                }
            )
    except (OSError, websockets.exceptions.InvalidHandshake) as connect_error:
        print(
            f"error: could not connect to {url}: {connect_error}\n"
            "Is the Python backend running? (uv run python psp.py --port 5174)",
            file=sys.stderr,
        )
        sys.exit(1)

    print(f"gps capture complete: {index} fixtures written to {corpus_dir}")


# ---------------------------------------------------------------------------
# Task 3E-3: the player-transfer scenario.
#
# Dynamic like `phase2`/`capture_gps`, not a FIXED_SCENARIOS list: it needs a
# REAL player uid from the corpus save (CORPUS_PLAYER_UID) to drive
# transfer_player, harvested from select_save's own get_player_summaries
# response burst -- the same harvest pattern `capture_phase2`/`capture_gps`
# use for their own live ids.
#
# The corpus save is loaded TWICE, in two different roles, over ONE
# connection: once as the ordinary MAIN save via `select_save` (so
# transfer_player has a target -- `ctx.session.save`), and once as the
# transfer SOURCE via `load_source_save` (role="source", same --save-dir).
# `transfer_player` is called with `target_player_uid: None` (spawn mode),
# spawning the source player into the main save under its own uid -- this
# never touches the standalone-target auto-save-to-disk branch
# (`handlers/tools.rs::handle_transfer_player`'s `has_standalone_target`
# path), which needs its OWN dedicated, disk-writing corpus and is
# deliberately NOT exercised here (see parity/README.md, "transfer
# scenario", for why and how that would be captured safely later).
#
# Wire-response determinism: every response this scenario captures is a
# fixed-shape success/error object (`{success, role, player_count,
# world_name}`, `{source: {...}, target: {...}}` keyed by REAL on-disk player
# uuids, `{success: true}`) -- no fresh uuid4 is minted on any code path this
# sequence exercises (spawn-mode transfer_player never returns an id; the
# guild `Uuid::new_v4()` `create_guild_for_player` may mint stays inside the
# target SAVE TREE, not the WS response). So, unlike add_pal/add_preset/
# add_gps_pal, this scenario needs NO PARITY_IGNORED_PATHS entries.
TRANSFER_SCENARIO = "transfer"


async def capture_transfer(
    url: str,
    save_dir: str,
    corpus: str,
    output_root: pathlib.Path,
) -> None:
    corpus_dir = output_root / corpus
    corpus_dir.mkdir(parents=True, exist_ok=True)
    level_sav_path = os.path.join(save_dir, "Level.sav")

    index = 0
    try:
        socket_context = websockets.connect(url, max_size=2**30)
        async with socket_context as socket:

            async def run(request: dict) -> list:
                nonlocal index
                await socket.send(json.dumps(request))
                responses = await _drain_response_burst(socket)
                _refuse_null_save_dir(request, responses)
                _write_fixture(corpus_dir, index, request, responses)
                index += 1
                return responses

            # 000 select_save -- load the corpus save as the MAIN save, so
            # transfer_player has a target. Harvest a real player uid for
            # CORPUS_PLAYER_UID.
            select_responses = await run(
                {
                    "type": "select_save",
                    "data": {"type": "steam", "path": level_sav_path, "local": False},
                }
            )
            player_summaries = _find_response(select_responses, "get_player_summaries")
            player_ids = (
                list(player_summaries["data"].keys()) if player_summaries else []
            )
            if not player_ids:
                print(
                    "error: corpus has no players -- the transfer scenario's "
                    "transfer_player step needs a real source_player_uid "
                    "(CORPUS_PLAYER_UID).",
                    file=sys.stderr,
                )
                sys.exit(1)
            corpus_player_uid = player_ids[0]
            print(f"transfer: corpus_player_uid={corpus_player_uid}")

            # 001 load_source_save -- unsupported save_type error path.
            await run(
                {
                    "type": "load_source_save",
                    "data": {"type": "gamepass", "path": "X", "role": "source"},
                }
            )
            # 002 get_source_players -- nothing loaded into source/target yet.
            await run({"type": "get_source_players", "data": None})
            # 003 load_source_save -- the real corpus save, as the transfer
            # SOURCE (role="source", same --save-dir as the main select_save
            # above -- a save can be both a source and, separately, the main
            # save at once; this scenario deliberately does NOT load it a
            # third time as a standalone role="target").
            await run(
                {
                    "type": "load_source_save",
                    "data": {"type": "steam", "path": save_dir, "role": "source"},
                }
            )
            # 004 get_source_players -- now source is populated, target still
            # empty (no standalone target loaded; transfer_player below falls
            # back to the main save).
            await run({"type": "get_source_players", "data": None})
            # 005 transfer_player -- spawn mode (target_player_uid: None):
            # spawns the source player into the main save under its own uid.
            await run(
                {
                    "type": "transfer_player",
                    "data": {
                        "source_player_uid": corpus_player_uid,
                        "target_player_uid": None,
                    },
                }
            )
            # 006 unload_source_save -- clears source + transfer_target.
            await run({"type": "unload_source_save", "data": None})
    except (OSError, websockets.exceptions.InvalidHandshake) as connect_error:
        print(
            f"error: could not connect to {url}: {connect_error}\n"
            "Is the Python backend running? (uv run python psp.py --port 5174)",
            file=sys.stderr,
        )
        sys.exit(1)

    print(f"transfer capture complete: {index} fixtures written to {corpus_dir}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--scenario",
        choices=sorted(
            set(FIXED_SCENARIOS)
            | set(SAVE_DIR_SCENARIOS)
            | {PHASE2_SCENARIO, GPS_SCENARIO, TRANSFER_SCENARIO}
        ),
        default="static-data",
        help="which request sequence to send",
    )
    parser.add_argument(
        "--corpus",
        default=None,
        help="output subdirectory name under --output; defaults to --scenario "
        "(set this explicitly to capture the same scenario against multiple "
        "corpus saves, e.g. --scenario load_path --corpus steam-1p)",
    )
    parser.add_argument(
        "--save-dir",
        default=None,
        help="corpus save's directory (Level.sav's parent) -- required for "
        "SAVE_DIR_SCENARIOS scenarios such as load_path. Pass an ABSOLUTE "
        "path: it is embedded verbatim in the captured select_save request "
        "and read from disk again during Rust replay, which may run from a "
        "different working directory than this script. For "
        "--scenario gamepass, this is the STEAM corpus save dir when paired "
        "with --prepare-gamepass, and the PRINTED container dir on the "
        "subsequent capturing run (see parity/README.md).",
    )
    parser.add_argument(
        "--prepare-gamepass",
        action="store_true",
        help="run the gamepass scenario's pre-backend-start step: build the "
        "synthetic wgs container tree from --save-dir (a steam corpus save "
        "dir) and persist its path into psp.db's settings row, then print "
        "the container dir and exit -- no backend connection is made. Must "
        "run and complete BEFORE the Python backend process starts (see "
        "parity/README.md, 'gamepass scenario').",
    )
    # psp.py:51 declares `client_id: int` on the WS route, so a non-numeric
    # path segment (e.g. "parity-capture") fails the Starlette route
    # converter and the handshake is rejected with HTTP 403 before this
    # script's socket even opens. Any fixed integer works; it is otherwise
    # unused by the Python backend.
    parser.add_argument("--url", default="ws://127.0.0.1:5174/ws/999999999")
    parser.add_argument(
        "--output",
        default=str(pathlib.Path(__file__).resolve().parents[1] / "parity/fixtures"),
    )
    arguments = parser.parse_args()
    corpus = arguments.corpus or arguments.scenario
    output_root = pathlib.Path(arguments.output)

    if arguments.prepare_gamepass:
        if arguments.scenario != "gamepass":
            print(
                "error: --prepare-gamepass only applies to --scenario gamepass",
                file=sys.stderr,
            )
            sys.exit(1)
        if arguments.save_dir is None:
            print(
                "error: --prepare-gamepass requires --save-dir (a STEAM corpus "
                "save's directory, i.e. Level.sav's parent -- tests/fixtures/"
                "saves/world2 in this checkout). Pass an ABSOLUTE path.",
                file=sys.stderr,
            )
            sys.exit(1)
        container_dir = prepare_gamepass_corpus(Path(arguments.save_dir))
        print(f"gamepass corpus prepared: {container_dir}")
        print(
            "Next: start the Python backend, then re-run this script WITHOUT "
            f"--prepare-gamepass and with --save-dir \"{container_dir}\" to capture."
        )
        return

    if arguments.scenario == PHASE2_SCENARIO:
        if arguments.save_dir is None:
            print(
                f"error: scenario {PHASE2_SCENARIO!r} requires --save-dir (the "
                "corpus save's directory, i.e. Level.sav's parent). Pass an "
                "ABSOLUTE path.",
                file=sys.stderr,
            )
            sys.exit(1)
        asyncio.run(
            capture_phase2(arguments.url, arguments.save_dir, corpus, output_root)
        )
        return

    if arguments.scenario == GPS_SCENARIO:
        if arguments.save_dir is None:
            print(
                f"error: scenario {GPS_SCENARIO!r} requires --save-dir (a corpus "
                "save's directory, i.e. Level.sav's parent, that ALSO has a "
                "GlobalPalStorage.sav). Pass an ABSOLUTE path.",
                file=sys.stderr,
            )
            sys.exit(1)
        asyncio.run(capture_gps(arguments.url, arguments.save_dir, corpus, output_root))
        return

    if arguments.scenario == TRANSFER_SCENARIO:
        if arguments.save_dir is None:
            print(
                f"error: scenario {TRANSFER_SCENARIO!r} requires --save-dir (the "
                "corpus save's directory, i.e. Level.sav's parent). Pass an "
                "ABSOLUTE path.",
                file=sys.stderr,
            )
            sys.exit(1)
        asyncio.run(
            capture_transfer(arguments.url, arguments.save_dir, corpus, output_root)
        )
        return

    asyncio.run(
        capture_corpus(
            arguments.url,
            arguments.scenario,
            corpus,
            arguments.save_dir,
            output_root,
        )
    )


if __name__ == "__main__":
    main()
