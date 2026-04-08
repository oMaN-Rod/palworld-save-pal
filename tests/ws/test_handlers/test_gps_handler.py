"""Tests for GPS WS handlers."""

from unittest.mock import AsyncMock, MagicMock, patch
from uuid import uuid4

import pytest

from palworld_save_pal.ws.messages import (
    AddPalData,
    AddPalMessage,
    BaseMessage,
    DeletePalsData,
    DeletePalsMessage,
    MessageType,
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