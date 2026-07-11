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


class TestStatusPointList1_0:
    """Palworld 1.0 added new status names (e.g. 移動速度アップ).

    Known 1.0 names must map to their snake_case keys; names the map doesn't
    know yet must be skipped instead of raising KeyError (issues #272/#276).
    """

    def _status_array(self, player):
        return PalObjects.get_array_property(
            player._save_parameter["GotStatusPointList"]
        )

    def test_move_speed_maps_to_snake_case(self, player_o):
        self._status_array(player_o).append(
            PalObjects.StatusPointStruct("移動速度アップ", 3)
        )
        assert player_o.status_point_list["move_speed"] == 3

    def test_unknown_status_name_is_skipped(self, player_o):
        self._status_array(player_o).append(
            PalObjects.StatusPointStruct("未知の新ステータス", 1)
        )
        spl = player_o.status_point_list  # must not raise
        assert 1 not in spl.values() or "未知の新ステータス" not in spl

    def test_setter_ignores_unknown_key(self, player_o):
        value = player_o.status_point_list
        value["totally_unknown_key"] = 5
        player_o.status_point_list = value  # must not raise
        assert "totally_unknown_key" not in player_o.status_point_list

    def test_getter_setter_getter_identity(self, player_o):
        self._status_array(player_o).append(
            PalObjects.StatusPointStruct("移動速度アップ", 2)
        )
        first = player_o.status_point_list
        player_o.status_point_list = first
        assert player_o.status_point_list == first

    def test_ext_unknown_status_name_is_skipped(self, player_o):
        ext_array = PalObjects.get_array_property(
            player_o._save_parameter["GotExStatusPointList"]
        )
        ext_array.append(PalObjects.StatusPointStruct("謎の拡張ステータス", 1))
        ext = player_o.ext_status_point_list  # must not raise
        assert "謎の拡張ステータス" not in ext
