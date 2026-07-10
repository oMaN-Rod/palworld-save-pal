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
# get_presets is EXCLUDED until Phase 3 (the Rust side is a stopgap).
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


# Scenarios with a fixed request list, independent of any on-disk save.
FIXED_SCENARIOS = {"static-data": STATIC_DATA_SCENARIO}

# Scenarios that build their request list from a corpus save directory
# (--save-dir), keyed by scenario name.
SAVE_DIR_SCENARIOS = {"load_path": load_path_requests}

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
                responses = []
                while True:
                    try:
                        frame = await asyncio.wait_for(socket.recv(), timeout=IDLE_SECONDS)
                    except asyncio.TimeoutError:
                        break
                    responses.append(json.loads(frame))

                offending_type = _null_save_dir_response_type(responses)
                if offending_type is not None:
                    # A truly fresh psp.db makes Python's settings loader hit
                    # a missing table, swallow the error, and report
                    # save_dir: null forever for that process's life. This is
                    # a known, 100%-reproducible capture-time artifact, not a
                    # legitimate Rust/Python divergence — recording it would
                    # be a trap for whoever replays this fixture next. Refuse
                    # to write it; see rust/parity/README.md, "Known Python
                    # quirks affecting capture", for the fix (warm the DB
                    # first) and why this must NOT become a
                    # PARITY_IGNORED_PATHS mask.
                    print(
                        f"error: response type {offending_type!r} (for request "
                        f"{request['type']!r}) has save_dir: null — the Python "
                        "backend's settings table did not exist when it started. "
                        "See 'Known Python quirks affecting capture' in "
                        "rust/parity/README.md: stop the backend, then start it "
                        "again (a warmed psp.db) before capturing. Refusing to "
                        "write this fixture.",
                        file=sys.stderr,
                    )
                    sys.exit(1)

                fixture_path = corpus_dir / f"{request_index:03d}_{request['type']}.json"
                fixture_path.write_text(
                    json.dumps(
                        {"request": request, "responses": responses},
                        indent=2,
                        ensure_ascii=False,
                    ),
                    encoding="utf-8",
                )
                print(f"wrote {fixture_path} ({len(responses)} responses)")
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


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--scenario",
        choices=sorted(set(FIXED_SCENARIOS) | set(SAVE_DIR_SCENARIOS)),
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
    asyncio.run(
        capture_corpus(
            arguments.url,
            arguments.scenario,
            corpus,
            arguments.save_dir,
            pathlib.Path(arguments.output),
        )
    )


if __name__ == "__main__":
    main()
