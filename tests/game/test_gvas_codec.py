import base64

import pytest

from palworld_save_pal.game.gvas_codec import CUSTOM_PROPERTIES, SaveType, _ensure_bytes


class TestSaveType:
    def test_steam_value(self):
        assert SaveType.STEAM == 0

    def test_gamepass_value(self):
        assert SaveType.GAMEPASS == 1

    def test_is_int_enum(self):
        assert isinstance(SaveType.STEAM.value, int)


class TestCustomProperties:
    def test_foliage_registered(self):
        assert ".worldSaveData.FoliageGridSaveDataMap" in CUSTOM_PROPERTIES

    def test_dungeon_registered(self):
        assert ".worldSaveData.DungeonSaveData" in CUSTOM_PROPERTIES

    def test_enemy_camp_registered(self):
        assert ".worldSaveData.EnemyCampSaveData" in CUSTOM_PROPERTIES

    def test_game_time_registered(self):
        assert ".worldSaveData.GameTimeSaveData" in CUSTOM_PROPERTIES

    def test_base_camp_module_map(self):
        assert (
            ".worldSaveData.BaseCampSaveData.Value.ModuleMap" in CUSTOM_PROPERTIES
        )

    def test_skip_properties_have_decode_encode_tuple(self):
        key = ".worldSaveData.FoliageGridSaveDataMap"
        handler = CUSTOM_PROPERTIES[key]
        assert isinstance(handler, tuple)
        assert len(handler) == 2
        assert callable(handler[0])
        assert callable(handler[1])

    def test_inherits_palworld_custom_properties(self):
        # Should include properties from palworld-save-tools
        assert len(CUSTOM_PROPERTIES) > 10


class TestEnsureBytes:
    def test_passes_bytes_through(self):
        assert _ensure_bytes(b"\x01\x02\x03") == b"\x01\x02\x03"

    def test_converts_bytearray(self):
        assert _ensure_bytes(bytearray(b"\x01\x02")) == b"\x01\x02"

    def test_decodes_base64_string(self):
        payload = b"\x00\x10\xffhello"
        encoded = base64.b64encode(payload).decode("ascii")
        assert _ensure_bytes(encoded) == payload

    def test_falls_back_to_hex_for_legacy_strings(self):
        payload = b"\x01\x02\x03\x04\x05\x06\x07\x08"
        encoded = payload.hex()
        assert _ensure_bytes(encoded) == payload

    def test_empty_string_returns_empty_bytes(self):
        assert _ensure_bytes("") == b""

    def test_list_of_ints_converted(self):
        assert _ensure_bytes([1, 2, 3]) == b"\x01\x02\x03"
