import pytest

from palworld_save_pal.game.gvas_codec import CUSTOM_PROPERTIES, SaveType


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
