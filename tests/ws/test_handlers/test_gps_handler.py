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