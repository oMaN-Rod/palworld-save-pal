"""Tests for player WS handler."""

from unittest.mock import AsyncMock, MagicMock, patch
from uuid import uuid4

import pytest

from palworld_save_pal.ws.messages import (
    DeletePlayerData,
    DeletePlayerMessage,
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


class TestDeletePlayerHandler:
    @pytest.mark.asyncio
    async def test_deletes_player(self, ws):
        from palworld_save_pal.ws.handlers.player_handler import delete_player_handler

        player_id = uuid4()
        mock_state = MagicMock()
        mock_state.save_file = MagicMock()
        mock_state.save_file.delete_player = AsyncMock(return_value=True)

        with patch(
            "palworld_save_pal.ws.handlers.player_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = DeletePlayerMessage(
                data=DeletePlayerData(player_id=player_id, origin="test")
            )
            await delete_player_handler(msg, ws)

        mock_state.save_file.delete_player.assert_called_once()
        responses = [s for s in ws.sent if s["type"] == MessageType.DELETE_PLAYER.value]
        assert len(responses) == 1
        assert responses[0]["data"]["player_id"] is not None

    @pytest.mark.asyncio
    async def test_delete_admin_player_returns_none_id(self, ws):
        from palworld_save_pal.ws.handlers.player_handler import delete_player_handler

        player_id = uuid4()
        mock_state = MagicMock()
        mock_state.save_file = MagicMock()
        mock_state.save_file.delete_player = AsyncMock(return_value=False)

        with patch(
            "palworld_save_pal.ws.handlers.player_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = DeletePlayerMessage(
                data=DeletePlayerData(player_id=player_id, origin="test")
            )
            await delete_player_handler(msg, ws)

        responses = [s for s in ws.sent if s["type"] == MessageType.DELETE_PLAYER.value]
        assert len(responses) == 1
        assert responses[0]["data"]["player_id"] is None

    @pytest.mark.asyncio
    async def test_no_save_file_returns_warning(self, ws):
        from palworld_save_pal.ws.handlers.player_handler import delete_player_handler

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.player_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = DeletePlayerMessage(
                data=DeletePlayerData(player_id=uuid4(), origin="test")
            )
            await delete_player_handler(msg, ws)

        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.WARNING.value