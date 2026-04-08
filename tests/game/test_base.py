"""Integration tests for Base entity using real save data."""

from uuid import UUID

import pytest

from palworld_save_pal.game.base import Base
from tests.game.conftest import PLAYER_O_UID, _noop


@pytest.fixture
def loaded_base(fresh_save_manager):
    """Load the guild that has a base, return the first base."""
    summaries = fresh_save_manager.get_guild_summaries()
    guild_id = None
    for gid, gs in summaries.items():
        if gs.base_count > 0:
            guild_id = gid
            break
    assert guild_id is not None
    fresh_save_manager._load_guild_by_id(guild_id)
    guild = fresh_save_manager._guilds[guild_id]
    base_id = list(guild.bases.keys())[0]
    return guild.bases[base_id]


class TestBaseProperties:
    def test_has_id(self, loaded_base):
        assert loaded_base.id is not None
        assert isinstance(loaded_base.id, UUID)

    def test_has_name(self, loaded_base):
        assert loaded_base.name is not None

    def test_has_container_id(self, loaded_base):
        assert loaded_base.container_id is not None

    def test_has_pal_container(self, loaded_base):
        assert loaded_base.pal_container is not None

    def test_area_range(self, loaded_base):
        # area_range should be a float
        assert isinstance(loaded_base.area_range, (int, float))


class TestBasePals:
    def test_base_has_pals(self, loaded_base):
        # The base may or may not have pals assigned
        assert isinstance(loaded_base.pals, dict)

    def test_base_pals_have_character_ids(self, loaded_base):
        for pal in loaded_base.pals.values():
            assert pal.character_id is not None


class TestBaseStorageContainers:
    def test_has_storage_containers(self, loaded_base):
        assert isinstance(loaded_base.storage_containers, dict)