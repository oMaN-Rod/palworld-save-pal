"""Integration tests for serialization roundtrip."""

import asyncio
from pathlib import Path

import pytest

from palworld_save_tools.gvas import GvasFile
from palworld_save_tools.palsav import compress_gvas_to_sav, decompress_sav_to_gvas
from palworld_save_tools.paltypes import PALWORLD_TYPE_HINTS

from palworld_save_pal.game.gvas_codec import CUSTOM_PROPERTIES
from tests.game.conftest import PLAYER_O_UID, WORLD1_DIR, _noop


class TestRoundtrip:
    def test_level_sav_roundtrip(self, fresh_save_manager):
        """Serialize Level.sav back and verify it can be re-parsed."""
        sav_bytes = fresh_save_manager.sav()
        assert isinstance(sav_bytes, bytes)
        assert len(sav_bytes) > 0

        # Re-parse the output
        raw_gvas, _ = decompress_sav_to_gvas(sav_bytes)
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        assert gvas_file is not None
        world_save_data = gvas_file.properties.get("worldSaveData")
        assert world_save_data is not None

    def test_player_gvas_roundtrip(self, event_loop, fresh_save_manager):
        """Load a player, serialize, and verify output is valid."""
        event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )
        player_files = fresh_save_manager.player_gvas_files()
        assert len(player_files) >= 1

        for uid, pf in player_files.items():
            sav_bytes = pf["sav"]
            assert sav_bytes is not None
            assert isinstance(sav_bytes, bytes)
            assert len(sav_bytes) > 0

            # Re-parse
            raw_gvas, _ = decompress_sav_to_gvas(sav_bytes)
            gvas_file = GvasFile.read(
                raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
            )
            assert gvas_file is not None
