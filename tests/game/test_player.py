"""Integration tests for Player entity using real save data."""

import asyncio
from uuid import UUID

import pytest

from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.game.player import Player, PlayerGvasFiles
from tests.game.conftest import PLAYER_O_UID, PLAYER_SKY_UID, _noop


@pytest.fixture
def player_o(event_loop, fresh_save_manager):
    return event_loop.run_until_complete(
        fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
    )


@pytest.fixture
def player_sky(event_loop, fresh_save_manager):
    return event_loop.run_until_complete(
        fresh_save_manager.load_player_on_demand(PLAYER_SKY_UID, ws_callback=_noop)
    )


class TestPlayerProperties:
    def test_nickname(self, player_o):
        assert player_o.nickname == "O"

    def test_level(self, player_o):
        assert player_o.level == 65

    def test_uid(self, player_o):
        assert player_o.uid == PLAYER_O_UID

    def test_pal_box_id(self, player_o):
        assert player_o.pal_box_id is not None
        assert isinstance(player_o.pal_box_id, UUID)

    def test_otomo_container_id(self, player_o):
        assert player_o.otomo_container_id is not None

    def test_instance_id(self, player_o):
        assert player_o.instance_id is not None

    def test_guild_id(self, player_o):
        assert player_o.guild_id is not None


class TestPlayerSky:
    def test_nickname(self, player_sky):
        assert player_sky.nickname == "sky"

    def test_level(self, player_sky):
        assert player_sky.level == 2


class TestPlayerWithoutRecordData:
    """Some player saves have no RecordData property; loading must not crash."""

    def test_loads_without_record_data(self, player_o):
        save_data = PalObjects.get_value(
            player_o._player_gvas_files.sav.properties["SaveData"]
        )
        save_data.pop("RecordData", None)

        gvas_files = PlayerGvasFiles(sav=player_o._player_gvas_files.sav, dps=None)
        player = Player(
            gvas_files=gvas_files,
            character_save_parameter=player_o._character_save,
        )

        assert player._record_data == {}

    def test_setters_persist_without_record_data(self, player_o):
        save_data = PalObjects.get_value(
            player_o._player_gvas_files.sav.properties["SaveData"]
        )
        save_data.pop("RecordData", None)

        gvas_files = PlayerGvasFiles(sav=player_o._player_gvas_files.sav, dps=None)
        player = Player(
            gvas_files=gvas_files,
            character_save_parameter=player_o._character_save,
        )

        player.collected_effigies = ["TestFlag"]

        # The created RecordData must be wired into SaveData so writes persist.
        assert "RecordData" in save_data
        assert player.collected_effigies == ["TestFlag"]


class TestPlayerContainers:
    def test_has_common_container(self, player_o):
        assert player_o.common_container is not None

    def test_has_essential_container(self, player_o):
        assert player_o.essential_container is not None
