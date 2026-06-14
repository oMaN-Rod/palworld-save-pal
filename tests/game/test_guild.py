"""Integration tests for Guild entity using real save data."""

import pytest

from palworld_save_pal.game.guild import Guild
from tests.game.conftest import PLAYER_O_UID, _noop


@pytest.fixture
def loaded_guild(event_loop, fresh_save_manager):
    """Load the first guild (the one with a base)."""
    summaries = fresh_save_manager.get_guild_summaries()
    # Find the guild with a base
    guild_id = None
    for gid, gs in summaries.items():
        if gs.base_count > 0:
            guild_id = gid
            break
    assert guild_id is not None, "No guild with base found"
    fresh_save_manager._load_guild_by_id(guild_id)
    return fresh_save_manager._guilds[guild_id]


class TestGuildProperties:
    def test_name(self, loaded_guild):
        assert loaded_guild.name is not None

    def test_has_players(self, loaded_guild):
        assert len(loaded_guild.players) >= 1

    def test_has_bases(self, loaded_guild):
        assert len(loaded_guild.bases) >= 1

    def test_base_has_id(self, loaded_guild):
        for base_id, base in loaded_guild.bases.items():
            assert base_id is not None
