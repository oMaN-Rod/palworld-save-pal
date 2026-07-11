#!/usr/bin/env python3
"""Capture WS request/response fixtures from the RUNNING Python backend.

The recorded fixtures are replayed against the Rust server by
rust/psp-server/tests/parity.rs. Fixtures are machine-local (save_dir defaults,
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
from typing import Optional

import websockets

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
    Level.sav, optional LevelMeta.sav, Players/*.sav at the archive root."""
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.write(os.path.join(save_dir, "Level.sav"), "Level.sav")
        level_meta = os.path.join(save_dir, "LevelMeta.sav")
        if os.path.exists(level_meta):
            archive.write(level_meta, "LevelMeta.sav")
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
# rust/parity/README.md, "db-presets scenario", for the safe capture
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
# See rust/parity/README.md, "db-ups scenario", for the safe capture procedure
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

# Scenarios with a fixed request list, independent of any on-disk save.
FIXED_SCENARIOS = {
    "static-data": STATIC_DATA_SCENARIO,
    DB_PRESETS_SCENARIO[0]: DB_PRESETS_SCENARIO[1],
    DB_UPS_SCENARIO[0]: DB_UPS_SCENARIO[1],
}

# Scenarios that build their request list from a corpus save directory
# (--save-dir), keyed by scenario name.
SAVE_DIR_SCENARIOS = {"load_path": load_path_requests}

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
    rust/parity/README.md) — not a general fixture validator."""
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
    # rust/parity/README.md, "Known Python quirks affecting capture", for the
    # fix (warm the DB first) and why this must NOT become a
    # PARITY_IGNORED_PATHS mask.
    print(
        f"error: response type {offending_type!r} (for request "
        f"{request['type']!r}) has save_dir: null — the Python backend's "
        "settings table did not exist when it started. See 'Known Python "
        "quirks affecting capture' in rust/parity/README.md: stop the backend, "
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


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--scenario",
        choices=sorted(set(FIXED_SCENARIOS) | set(SAVE_DIR_SCENARIOS) | {PHASE2_SCENARIO}),
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
        "different working directory than this script.",
    )
    # psp.py:51 declares `client_id: int` on the WS route, so a non-numeric
    # path segment (e.g. "parity-capture") fails the Starlette route
    # converter and the handshake is rejected with HTTP 403 before this
    # script's socket even opens. Any fixed integer works; it is otherwise
    # unused by the Python backend.
    parser.add_argument("--url", default="ws://127.0.0.1:5174/ws/999999999")
    parser.add_argument(
        "--output",
        default=str(pathlib.Path(__file__).resolve().parents[1] / "rust/parity/fixtures"),
    )
    arguments = parser.parse_args()
    corpus = arguments.corpus or arguments.scenario
    output_root = pathlib.Path(arguments.output)

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
