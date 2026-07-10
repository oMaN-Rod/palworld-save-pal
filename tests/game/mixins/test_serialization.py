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


class TestRepeatedSerialization:
    """SaveManager caches GvasFile objects, so serializing must not consume them.

    These guard the contract that lets sav() skip a deepcopy of the whole Level
    GVAS. They require a palworld-save-tools whose rawdata encoders do not
    mutate their `properties` argument.
    """

    def test_sav_twice_produces_identical_bytes(self, fresh_save_manager):
        first = fresh_save_manager.sav()
        second = fresh_save_manager.sav()
        assert first == second

    def test_sav_preserves_decoded_custom_properties(self, fresh_save_manager):
        world = fresh_save_manager._gvas_file.properties["worldSaveData"]["value"]
        group_map = world["GroupSaveDataMap"]
        foliage = world["FoliageGridSaveDataMap"]
        map_objects = world["MapObjectSaveData"]

        fresh_save_manager.sav()

        # Library encoder (group.encode)
        assert "custom_type" in group_map
        assert "group_id" in group_map["value"][0]["value"]["RawData"]["value"]
        # skip_encode path
        assert "custom_type" in foliage
        assert "skip_type" in foliage
        # Deeply nested encoder (map_object.encode)
        assert "custom_type" in map_objects
        first_object = map_objects["value"]["values"][0]
        assert "values" not in first_object["Model"]["value"]["RawData"]["value"]
