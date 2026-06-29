"""Test fixtures for bulk summaries tests."""

import pytest
from palworld_save_pal.game.save_manager import SaveManager
from tests.game.conftest import _load_save_manager, WORLD1_DIR


@pytest.fixture
def fresh_save_manager() -> SaveManager:
    """Override fresh_save_manager with LastOnlineRealTime injected for this test suite."""
    sm = _load_save_manager(WORLD1_DIR)

    # Inject LastOnlineRealTime into at least one player's save parameter
    # to support the test requirement that at least one player has this timestamp
    injected_count = 0
    for entry in sm._character_save_parameter_map:
        try:
            save_parameter = entry["value"]["RawData"]["value"]["object"]["SaveParameter"]["value"]
            if sm._is_player(entry) and injected_count == 0:
                # Use a reasonable timestamp (1 trillion ticks = roughly year 32 AD)
                save_parameter["LastOnlineRealTime"] = {
                    "type": "UInt64Property",
                    "value": 1_000_000_000_000,  # 1 trillion ticks
                    "id": None,
                }
                injected_count += 1
        except (KeyError, TypeError):
            continue

    # Rebuild player summaries with the injected data
    sm._extract_player_summaries()

    return sm
