"""Extended tests for LoadingMixin using real save data."""

from uuid import UUID, uuid4

import pytest

from palworld_save_pal.game.guild import Guild
from palworld_save_pal.game.pal import Pal
from tests.game.conftest import PLAYER_O_UID, PLAYER_SKY_UID, GPS_FILE, _noop


class TestLoadGuildById:
    def test_load_guild_returns_guild(self, fresh_save_manager):
        guild_id = list(fresh_save_manager.get_guild_summaries().keys())[0]
        guild = fresh_save_manager._load_guild_by_id(guild_id)
        assert guild is not None
        assert isinstance(guild, Guild)

    def test_load_guild_cached(self, fresh_save_manager):
        guild_id = list(fresh_save_manager.get_guild_summaries().keys())[0]
        g1 = fresh_save_manager._load_guild_by_id(guild_id)
        g2 = fresh_save_manager._load_guild_by_id(guild_id)
        assert g1 is g2

    def test_load_guild_not_found(self, fresh_save_manager):
        result = fresh_save_manager._load_guild_by_id(uuid4())
        assert result is None

    def test_load_guild_marks_loaded(self, fresh_save_manager):
        guild_id = list(fresh_save_manager.get_guild_summaries().keys())[0]
        assert fresh_save_manager.is_guild_loaded(guild_id) is False
        fresh_save_manager._load_guild_by_id(guild_id)
        assert fresh_save_manager.is_guild_loaded(guild_id) is True

    def test_load_guild_updates_summary(self, fresh_save_manager):
        guild_id = list(fresh_save_manager.get_guild_summaries().keys())[0]
        summary = fresh_save_manager.get_guild_summaries()[guild_id]
        assert summary.loaded is False
        fresh_save_manager._load_guild_by_id(guild_id)
        assert summary.loaded is True


class TestLoadBasesForGuild:
    def test_guild_with_base_has_bases_loaded(self, fresh_save_manager):
        summaries = fresh_save_manager.get_guild_summaries()
        guild_with_base = None
        for gid, gs in summaries.items():
            if gs.base_count > 0:
                guild_with_base = gid
                break
        assert guild_with_base is not None

        fresh_save_manager._load_guild_by_id(guild_with_base)
        guild = fresh_save_manager._guilds[guild_with_base]
        assert len(guild.bases) > 0

    def test_guild_without_base_has_no_bases(self, fresh_save_manager):
        summaries = fresh_save_manager.get_guild_summaries()
        guild_without_base = None
        for gid, gs in summaries.items():
            if gs.base_count == 0:
                guild_without_base = gid
                break
        if guild_without_base:
            fresh_save_manager._load_guild_by_id(guild_without_base)
            guild = fresh_save_manager._guilds[guild_without_base]
            assert len(guild.bases) == 0


class TestLoadPlayerPalsOnly:
    def test_loads_pals_for_player(self, event_loop, fresh_save_manager):
        event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )
        # Pals should already be in _pals
        assert len(fresh_save_manager._pals) == 11

    def test_no_pals_for_player_with_no_pals(self, event_loop, fresh_save_manager):
        event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_SKY_UID, ws_callback=_noop)
        )
        # Sky has 0 pals
        sky_pals = {
            k: v
            for k, v in fresh_save_manager._pals.items()
            if str(v.owner_uid) == str(PLAYER_SKY_UID)
        }
        assert len(sky_pals) == 0


class TestLoadGpsPals:
    def test_load_gps_pals(self, fresh_save_manager):
        with open(GPS_FILE, "rb") as f:
            gps_data = f.read()
        gps_pals = fresh_save_manager.load_gps(gps_data)
        assert len(gps_pals) == 6
        for idx, pal in gps_pals.items():
            assert isinstance(pal, Pal)
            assert isinstance(idx, int)

    def test_gps_pals_have_character_ids(self, fresh_save_manager):
        with open(GPS_FILE, "rb") as f:
            gps_data = f.read()
        gps_pals = fresh_save_manager.load_gps(gps_data)
        for pal in gps_pals.values():
            assert pal.character_id is not None


class TestPlayerNotFound:
    def test_load_player_not_in_refs(self, event_loop, fresh_save_manager):
        result = event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(uuid4(), ws_callback=_noop)
        )
        assert result is None


class TestIsPlayerLoaded:
    def test_not_loaded_initially(self, fresh_save_manager):
        assert fresh_save_manager.is_player_loaded(PLAYER_O_UID) is False

    def test_loaded_after_demand(self, event_loop, fresh_save_manager):
        event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )
        assert fresh_save_manager.is_player_loaded(PLAYER_O_UID) is True


class TestIsGuildLoaded:
    def test_not_loaded_initially(self, fresh_save_manager):
        guild_id = list(fresh_save_manager.get_guild_summaries().keys())[0]
        assert fresh_save_manager.is_guild_loaded(guild_id) is False

    def test_loaded_after_load(self, fresh_save_manager):
        guild_id = list(fresh_save_manager.get_guild_summaries().keys())[0]
        fresh_save_manager._load_guild_by_id(guild_id)
        assert fresh_save_manager.is_guild_loaded(guild_id) is True