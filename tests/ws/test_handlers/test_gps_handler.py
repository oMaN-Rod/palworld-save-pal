"""Tests for GPS WS handlers."""

from unittest.mock import AsyncMock, MagicMock, patch
from uuid import uuid4

import pytest

from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.game.enum import PalGender
from palworld_save_pal.ws.messages import (
    AddPalData,
    AddPalMessage,
    BaseMessage,
    CloneGpsPalData,
    CloneGpsPalMessage,
    CloneGpsPalToPlayerData,
    CloneGpsPalToPlayerMessage,
    DeletePalsData,
    DeletePalsMessage,
    MessageType,
)


def _make_pal_dto() -> PalDTO:
    return PalDTO(
        instance_id=uuid4(),
        owner_uid=uuid4(),
        character_id="Lambball",
        is_lucky=False,
        is_boss=False,
        gender=PalGender.FEMALE,
        rank_hp=0, rank_attack=0, rank_defense=0, rank_craftspeed=0,
        talent_hp=50, talent_shot=50, talent_defense=50,
        rank=1, level=10, exp=0,
        nickname="TestPal",
        is_tower=False,
        storage_id=uuid4(),
        stomach=300.0,
        storage_slot=0,
        learned_skills=[], active_skills=[], passive_skills=[],
        hp=5000, max_hp=5000,
        group_id=uuid4(),
        sanity=100.0,
        work_suitability={},
        is_sick=False,
        friendship_point=0,
    )


class MockWebSocket:
    def __init__(self):
        self.sent = []

    async def send_json(self, data):
        self.sent.append(data)


@pytest.fixture
def ws():
    return MockWebSocket()


@pytest.fixture
def mock_app_state():
    state = MagicMock()
    state.save_file = MagicMock()
    return state


class TestAddGpsPalHandler:
    @pytest.mark.asyncio
    async def test_adds_gps_pal(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.gps_handler import add_gps_pal_handler

        mock_pal = MagicMock()
        mock_app_state.save_file.add_gps_pal.return_value = (mock_pal, 0)

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = AddPalMessage(
                type=MessageType.ADD_GPS_PAL.value,
                data=AddPalData(character_id="Lambball", nickname="Test"),
            )
            await add_gps_pal_handler(msg, ws)

        mock_app_state.save_file.add_gps_pal.assert_called_once()
        assert len(ws.sent) == 1


class TestCloneGpsPalHandler:
    @pytest.mark.asyncio
    async def test_clones_gps_pal(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.gps_handler import clone_gps_pal_handler

        mock_pal = MagicMock()
        mock_app_state.save_file.clone_gps_pal.return_value = (3, mock_pal)

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = CloneGpsPalMessage(data=CloneGpsPalData(pal=_make_pal_dto()))
            await clone_gps_pal_handler(msg, ws)

        mock_app_state.save_file.clone_gps_pal.assert_called_once()
        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.ADD_GPS_PAL.value
        assert ws.sent[0]["data"]["index"] == 3
        assert "pal" in ws.sent[0]["data"]

    @pytest.mark.asyncio
    async def test_clone_failure_sends_error(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.gps_handler import clone_gps_pal_handler

        mock_app_state.save_file.clone_gps_pal.return_value = None

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = CloneGpsPalMessage(data=CloneGpsPalData(pal=_make_pal_dto()))
            await clone_gps_pal_handler(msg, ws)

        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.ADD_GPS_PAL.value
        assert "error" in ws.sent[0]["data"]

    @pytest.mark.asyncio
    async def test_no_save_file_returns_early(self, ws):
        from palworld_save_pal.ws.handlers.gps_handler import clone_gps_pal_handler

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = CloneGpsPalMessage(data=CloneGpsPalData(pal=_make_pal_dto()))
            await clone_gps_pal_handler(msg, ws)

        assert len(ws.sent) == 0


class TestDeleteGpsPalsHandler:
    @pytest.mark.asyncio
    async def test_deletes_gps_pals(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.gps_handler import delete_gps_pals_handler

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = DeletePalsMessage(
                type=MessageType.DELETE_GPS_PALS.value,
                data=DeletePalsData(pal_indexes=[0, 1]),
            )
            await delete_gps_pals_handler(msg, ws)

        mock_app_state.save_file.delete_gps_pals.assert_called_once()

    @pytest.mark.asyncio
    async def test_no_save_file_returns_early(self, ws):
        from palworld_save_pal.ws.handlers.gps_handler import delete_gps_pals_handler

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = DeletePalsMessage(
                type=MessageType.DELETE_GPS_PALS.value,
                data=DeletePalsData(pal_indexes=[0]),
            )
            # Should not crash
            await delete_gps_pals_handler(msg, ws)


class TestRequestGpsHandler:
    @pytest.mark.asyncio
    async def test_request_gps_no_save(self, ws):
        from palworld_save_pal.ws.handlers.gps_handler import request_gps_handler

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = BaseMessage(type=MessageType.REQUEST_GPS.value)
            await request_gps_handler(msg, ws)

        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.GET_GPS_RESPONSE.value


class TestCloneGpsPalToPlayerHandler:
    def _setup(self, mock_app_state, destination_type, add_return):
        player_uid = uuid4()
        pal_uuid = uuid4()

        player = MagicMock()
        player.uid = player_uid
        player.pal_box_id = uuid4()

        gps_pal = MagicMock()
        gps_pal.instance_id = pal_uuid

        save_file = mock_app_state.save_file
        save_file.get_players.return_value = {player_uid: player}
        save_file.get_gps.return_value = {0: gps_pal}
        if destination_type == "pal_box":
            save_file.add_player_pal_from_dto.return_value = add_return
        else:
            save_file.add_player_dps_pal_from_dto.return_value = add_return
        return player_uid, pal_uuid

    @pytest.mark.asyncio
    async def test_clones_to_pal_box(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.gps_handler import (
            clone_gps_pal_to_player_handler,
        )

        new_pal = MagicMock()
        new_pal.model_dump.return_value = {"instance_id": "new-id"}
        player_uid, pal_uuid = self._setup(mock_app_state, "pal_box", new_pal)

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_app_state,
        ), patch(
            "palworld_save_pal.ws.handlers.gps_handler.PalDTO"
        ) as mock_dto:
            mock_dto.from_dict.return_value = MagicMock()
            msg = CloneGpsPalToPlayerMessage(
                data=CloneGpsPalToPlayerData(
                    pal_ids=[str(pal_uuid)],
                    destination_type="pal_box",
                    destination_player_uid=str(player_uid),
                )
            )
            await clone_gps_pal_to_player_handler(msg, ws)

        mock_app_state.save_file.add_player_pal_from_dto.assert_called_once()
        assert ws.sent[0]["type"] == MessageType.ADD_PAL.value
        assert ws.sent[0]["data"]["player_id"] == str(player_uid)
        assert ws.sent[-1]["type"] == MessageType.CLONE_GPS_PAL_TO_PLAYER.value
        assert ws.sent[-1]["data"]["cloned_count"] == 1
        assert ws.sent[-1]["data"]["success"] is True

    @pytest.mark.asyncio
    async def test_clones_to_dps(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.gps_handler import (
            clone_gps_pal_to_player_handler,
        )

        new_pal = MagicMock()
        new_pal.model_dump.return_value = {"instance_id": "new-id"}
        player_uid, pal_uuid = self._setup(mock_app_state, "dps", (5, new_pal))

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_app_state,
        ), patch(
            "palworld_save_pal.ws.handlers.gps_handler.PalDTO"
        ) as mock_dto:
            mock_dto.from_dict.return_value = MagicMock()
            msg = CloneGpsPalToPlayerMessage(
                data=CloneGpsPalToPlayerData(
                    pal_ids=[str(pal_uuid)],
                    destination_type="dps",
                    destination_player_uid=str(player_uid),
                )
            )
            await clone_gps_pal_to_player_handler(msg, ws)

        mock_app_state.save_file.add_player_dps_pal_from_dto.assert_called_once()
        assert ws.sent[0]["type"] == MessageType.ADD_DPS_PAL.value
        assert ws.sent[0]["data"]["player_id"] == str(player_uid)
        assert ws.sent[0]["data"]["index"] == 5
        assert ws.sent[-1]["type"] == MessageType.CLONE_GPS_PAL_TO_PLAYER.value
        assert ws.sent[-1]["data"]["cloned_count"] == 1
        assert ws.sent[-1]["data"]["success"] is True

    @pytest.mark.asyncio
    async def test_add_failure_recorded_as_error(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.gps_handler import (
            clone_gps_pal_to_player_handler,
        )

        player_uid, pal_uuid = self._setup(mock_app_state, "pal_box", None)

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_app_state,
        ), patch(
            "palworld_save_pal.ws.handlers.gps_handler.PalDTO"
        ) as mock_dto:
            mock_dto.from_dict.return_value = MagicMock()
            msg = CloneGpsPalToPlayerMessage(
                data=CloneGpsPalToPlayerData(
                    pal_ids=[str(pal_uuid)],
                    destination_type="pal_box",
                    destination_player_uid=str(player_uid),
                )
            )
            await clone_gps_pal_to_player_handler(msg, ws)

        # No ADD_PAL emitted; only the summary
        assert all(s["type"] != MessageType.ADD_PAL.value for s in ws.sent)
        assert ws.sent[-1]["type"] == MessageType.CLONE_GPS_PAL_TO_PLAYER.value
        assert ws.sent[-1]["data"]["cloned_count"] == 0
        assert ws.sent[-1]["data"]["success"] is False
        assert len(ws.sent[-1]["data"]["errors"]) == 1

    @pytest.mark.asyncio
    async def test_no_save_file_sends_error(self, ws):
        from palworld_save_pal.ws.handlers.gps_handler import (
            clone_gps_pal_to_player_handler,
        )

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.gps_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = CloneGpsPalToPlayerMessage(
                data=CloneGpsPalToPlayerData(
                    pal_ids=[str(uuid4())],
                    destination_type="pal_box",
                    destination_player_uid=str(uuid4()),
                )
            )
            await clone_gps_pal_to_player_handler(msg, ws)

        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.ERROR.value
