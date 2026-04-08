"""Integration tests for SaveManager using real save files."""

import asyncio
from uuid import UUID

import pytest

from palworld_save_pal.game.save_manager import SaveManager
from tests.game.conftest import PLAYER_O_UID, PLAYER_SKY_UID, _noop


class TestSaveManagerLoad:
    def test_world_name(self, save_manager_world1):
        assert save_manager_world1.world_name == "Autosave_W"

    def test_player_summaries_count(self, save_manager_world1):
        summaries = save_manager_world1.get_player_summaries()
        assert len(summaries) == 2

    def test_player_o_summary(self, save_manager_world1):
        summaries = save_manager_world1.get_player_summaries()
        assert PLAYER_O_UID in summaries
        ps = summaries[PLAYER_O_UID]
        assert ps.nickname == "O"
        assert ps.level == 65
        assert ps.pal_count == 11

    def test_player_sky_summary(self, save_manager_world1):
        summaries = save_manager_world1.get_player_summaries()
        assert PLAYER_SKY_UID in summaries
        ps = summaries[PLAYER_SKY_UID]
        assert ps.nickname == "sky"
        assert ps.level == 2

    def test_guild_summaries(self, save_manager_world1):
        summaries = save_manager_world1.get_guild_summaries()
        assert len(summaries) == 2
        for gs in summaries.values():
            assert gs.name is not None
            assert gs.player_count >= 1

    def test_guild_with_base(self, save_manager_world1):
        summaries = save_manager_world1.get_guild_summaries()
        has_base = any(gs.base_count > 0 for gs in summaries.values())
        assert has_base

    def test_character_save_parameters(self, save_manager_world1):
        assert len(save_manager_world1._character_save_parameter_map) == 13

    def test_group_save_data(self, save_manager_world1):
        assert len(save_manager_world1._group_save_data_map) == 8

    def test_size_set(self, save_manager_world1):
        assert save_manager_world1.size > 0

    def test_world2_loads(self, save_manager_world2):
        summaries = save_manager_world2.get_player_summaries()
        assert len(summaries) == 1


class TestSaveManagerPlayerLoad:
    def test_load_player_on_demand(self, event_loop, fresh_save_manager):
        player = event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )
        assert player is not None
        assert player.nickname == "O"
        assert player.level == 65

    def test_player_pals_loaded(self, event_loop, fresh_save_manager):
        event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )
        assert len(fresh_save_manager._pals) == 11

    def test_player_cached_on_second_load(self, event_loop, fresh_save_manager):
        p1 = event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )
        p2 = event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )
        assert p1 is p2

    def test_is_player_loaded(self, event_loop, fresh_save_manager):
        assert fresh_save_manager.is_player_loaded(PLAYER_O_UID) is False
        event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )
        assert fresh_save_manager.is_player_loaded(PLAYER_O_UID) is True


class TestSaveManagerGuildLoad:
    def test_load_guild(self, event_loop, fresh_save_manager):
        guild_id = list(fresh_save_manager.get_guild_summaries().keys())[0]
        fresh_save_manager._load_guild_by_id(guild_id)
        assert guild_id in fresh_save_manager._guilds
        guild = fresh_save_manager._guilds[guild_id]
        assert guild.name is not None

    def test_is_guild_loaded(self, event_loop, fresh_save_manager):
        guild_id = list(fresh_save_manager.get_guild_summaries().keys())[0]
        assert fresh_save_manager.is_guild_loaded(guild_id) is False
        fresh_save_manager._load_guild_by_id(guild_id)
        assert fresh_save_manager.is_guild_loaded(guild_id) is True


class TestSaveManagerIndexing:
    def test_character_containers_indexed(self, save_manager_world1):
        containers = save_manager_world1._get_character_containers()
        assert containers is not None
        assert len(containers.index) > 0

    def test_item_containers_indexed(self, save_manager_world1):
        containers = save_manager_world1._get_item_containers()
        assert containers is not None
        assert len(containers.index) > 0

    def test_dynamic_items_indexed(self, save_manager_world1):
        items = save_manager_world1._get_dynamic_items()
        assert items is not None

    def test_character_save_parameters_indexed(self, save_manager_world1):
        params = save_manager_world1._get_character_save_parameters()
        assert params is not None
        assert len(params.index) > 0


class TestSaveManagerWorldName:
    def test_set_world_name(self, fresh_save_manager):
        fresh_save_manager.set_world_name("TestWorld")
        assert fresh_save_manager.world_name == "TestWorld"

    def test_set_world_name_no_meta_raises(self):
        sm = SaveManager()
        with pytest.raises(ValueError, match="No LevelMeta"):
            sm.set_world_name("Test")
