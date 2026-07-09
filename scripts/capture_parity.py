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
import json
import pathlib
import sys

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

SCENARIOS = {"static-data": STATIC_DATA_SCENARIO}

# A request's response burst is considered complete after this much silence.
IDLE_SECONDS = 2.0


async def capture_corpus(url: str, corpus: str, output_root: pathlib.Path) -> None:
    corpus_dir = output_root / corpus
    corpus_dir.mkdir(parents=True, exist_ok=True)
    try:
        socket_context = websockets.connect(url, max_size=2**30)
        async with socket_context as socket:
            for request_index, request in enumerate(SCENARIOS[corpus]):
                await socket.send(json.dumps(request))
                responses = []
                while True:
                    try:
                        frame = await asyncio.wait_for(socket.recv(), timeout=IDLE_SECONDS)
                    except asyncio.TimeoutError:
                        break
                    responses.append(json.loads(frame))
                fixture_path = corpus_dir / f"{request_index:02d}_{request['type']}.json"
                fixture_path.write_text(
                    json.dumps(
                        {"request": request, "responses": responses},
                        indent=2,
                        ensure_ascii=False,
                    ),
                    encoding="utf-8",
                )
                print(f"wrote {fixture_path} ({len(responses)} responses)")
    except OSError as connect_error:
        # websockets raises a plain OSError (e.g. ConnectionRefusedError) when
        # nothing is listening at --url. Fail loudly instead of silently
        # writing zero fixtures and exiting 0 — a developer running this
        # against a stopped backend must see a clear error, not a quiet no-op.
        print(
            f"error: could not connect to {url}: {connect_error}\n"
            "Is the Python backend running? (uv run python psp.py --port 5174)",
            file=sys.stderr,
        )
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", choices=sorted(SCENARIOS), default="static-data")
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
    asyncio.run(capture_corpus(arguments.url, arguments.corpus, pathlib.Path(arguments.output)))


if __name__ == "__main__":
    main()
