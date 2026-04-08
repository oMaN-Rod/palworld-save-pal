"""Integration tests for GuildOpsMixin using real save data."""

from uuid import UUID, uuid4

import pytest

from palworld_save_pal.dto.guild import GuildDTO
from palworld_save_pal.game.guild import Guild
from tests.game.conftest import PLAYER_O_UID, _noop


@pytest.fixture
def sm_with_guilds(fresh_save_manager):
    """Load all guilds for testing."""
    guild_ids = list(fresh_save_manager.get_guild_summaries().keys())
    for gid in guild_ids:
        fresh_save_manager._load_guild_by_id(gid)
    return fresh_save_manager


@pytest.fixture
def guild_with_base_id(sm_with_guilds):
    """Return (save_manager, guild_id) for a guild that has at least one base."""
    for gid, guild in sm_with_guilds._guilds.items():
        if guild.bases:
            return sm_with_guilds, gid
    pytest.skip("No guild with bases found in test data")


# ---------------------------------------------------------------------------
# get_guild
# ---------------------------------------------------------------------------
class TestGetGuild:
    def test_get_guild_existing(self, sm_with_guilds):
        guild_id = next(iter(sm_with_guilds._guilds.keys()))
        guild = sm_with_guilds.get_guild(guild_id)
        assert guild is not None
        assert isinstance(guild, Guild)

    def test_get_guild_has_name(self, sm_with_guilds):
        guild_id = next(iter(sm_with_guilds._guilds.keys()))
        guild = sm_with_guilds.get_guild(guild_id)
        assert guild.name is not None
        assert isinstance(guild.name, str)
        assert len(guild.name) > 0

    def test_get_guild_has_id(self, sm_with_guilds):
        guild_id = next(iter(sm_with_guilds._guilds.keys()))
        guild = sm_with_guilds.get_guild(guild_id)
        assert guild.id == guild_id

    def test_get_guild_has_players(self, sm_with_guilds):
        guild_id = next(iter(sm_with_guilds._guilds.keys()))
        guild = sm_with_guilds.get_guild(guild_id)
        assert isinstance(guild.players, list)
        assert len(guild.players) >= 1

    def test_get_guild_nonexistent(self, sm_with_guilds):
        result = sm_with_guilds.get_guild(uuid4())
        assert result is None

    def test_get_guild_empty_uuid(self, sm_with_guilds):
        empty = UUID("00000000-0000-0000-0000-000000000000")
        result = sm_with_guilds.get_guild(empty)
        assert result is None

    def test_get_guild_before_load(self, fresh_save_manager):
        guild_ids = list(fresh_save_manager.get_guild_summaries().keys())
        # Guilds exist in summaries but haven't been fully loaded
        result = fresh_save_manager.get_guild(guild_ids[0])
        assert result is None


# ---------------------------------------------------------------------------
# get_guilds
# ---------------------------------------------------------------------------
class TestGetGuilds:
    def test_get_guilds_returns_dict(self, sm_with_guilds):
        guilds = sm_with_guilds.get_guilds()
        assert isinstance(guilds, dict)

    def test_get_guilds_count(self, sm_with_guilds):
        guilds = sm_with_guilds.get_guilds()
        assert len(guilds) == 2

    def test_get_guilds_values_are_guild_instances(self, sm_with_guilds):
        guilds = sm_with_guilds.get_guilds()
        for gid, guild in guilds.items():
            assert isinstance(gid, UUID)
            assert isinstance(guild, Guild)

    def test_get_guilds_empty_before_load(self, fresh_save_manager):
        guilds = fresh_save_manager.get_guilds()
        assert len(guilds) == 0

    def test_get_guilds_each_has_admin(self, sm_with_guilds):
        guilds = sm_with_guilds.get_guilds()
        for guild in guilds.values():
            assert guild.admin_player_uid is not None
            assert isinstance(guild.admin_player_uid, UUID)


# ---------------------------------------------------------------------------
# get_base
# ---------------------------------------------------------------------------
class TestGetBase:
    def test_get_base_existing(self, guild_with_base_id):
        sm, guild_id = guild_with_base_id
        guild = sm.get_guild(guild_id)
        base_id = next(iter(guild.bases.keys()))
        base = sm.get_base(base_id)
        assert base is not None
        assert base.id == base_id

    def test_get_base_nonexistent(self, sm_with_guilds):
        result = sm_with_guilds.get_base(uuid4())
        assert result is None

    def test_get_base_empty_uuid(self, sm_with_guilds):
        empty = UUID("00000000-0000-0000-0000-000000000000")
        result = sm_with_guilds.get_base(empty)
        assert result is None

    def test_get_base_searches_all_guilds(self, guild_with_base_id):
        sm, guild_id = guild_with_base_id
        guild = sm.get_guild(guild_id)
        base_id = next(iter(guild.bases.keys()))
        # get_base should find it even without specifying guild
        base = sm.get_base(base_id)
        assert base is not None


# ---------------------------------------------------------------------------
# update_guilds (async)
# ---------------------------------------------------------------------------
class TestUpdateGuilds:
    def test_update_guild_name(self, event_loop, sm_with_guilds):
        guild_id = next(iter(sm_with_guilds._guilds.keys()))
        dto = GuildDTO(name="RenamedGuild")
        event_loop.run_until_complete(
            sm_with_guilds.update_guilds({guild_id: dto}, ws_callback=_noop)
        )
        updated = sm_with_guilds.get_guild(guild_id)
        assert updated.name == "RenamedGuild"

    def test_update_guilds_empty_dict(self, event_loop, sm_with_guilds):
        original_names = {
            gid: g.name for gid, g in sm_with_guilds._guilds.items()
        }
        event_loop.run_until_complete(
            sm_with_guilds.update_guilds({}, ws_callback=_noop)
        )
        for gid, name in original_names.items():
            assert sm_with_guilds.get_guild(gid).name == name

    def test_update_guilds_no_gvas_raises(self, event_loop, fresh_save_manager):
        fresh_save_manager._gvas_file = None
        dto = GuildDTO(name="X")
        with pytest.raises(ValueError, match="No GvasFile"):
            event_loop.run_until_complete(
                fresh_save_manager.update_guilds({uuid4(): dto}, ws_callback=_noop)
            )

    def test_update_guild_base_camp_level(self, event_loop, sm_with_guilds):
        guild_id = next(iter(sm_with_guilds._guilds.keys()))
        dto = GuildDTO(base_camp_level=10)
        event_loop.run_until_complete(
            sm_with_guilds.update_guilds({guild_id: dto}, ws_callback=_noop)
        )
        updated = sm_with_guilds.get_guild(guild_id)
        assert updated.base_camp_level == 10

    def test_update_multiple_guilds(self, event_loop, sm_with_guilds):
        guild_ids = list(sm_with_guilds._guilds.keys())
        if len(guild_ids) < 2:
            pytest.skip("Need at least 2 guilds")
        modifications = {
            guild_ids[0]: GuildDTO(name="Guild_A"),
            guild_ids[1]: GuildDTO(name="Guild_B"),
        }
        event_loop.run_until_complete(
            sm_with_guilds.update_guilds(modifications, ws_callback=_noop)
        )
        assert sm_with_guilds.get_guild(guild_ids[0]).name == "Guild_A"
        assert sm_with_guilds.get_guild(guild_ids[1]).name == "Guild_B"
