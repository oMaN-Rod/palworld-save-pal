"""Tests for guild WS handler."""

from unittest.mock import AsyncMock, MagicMock, patch
from uuid import uuid4

import pytest

from palworld_save_pal.ws.messages import (
    DeleteGuildData,
    DeleteGuildMessage,
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


class TestDeleteGuildHandler:
    @pytest.mark.asyncio
    async def test_deletes_guild(self, ws):
        from palworld_save_pal.ws.handlers.guild_handler import delete_guild_handler

        guild_id = uuid4()
        mock_state = MagicMock()
        mock_state.save_file = MagicMock()
        mock_state.save_file.delete_guild_and_players = AsyncMock()

        with patch(
            "palworld_save_pal.ws.handlers.guild_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = DeleteGuildMessage(
                data=DeleteGuildData(guild_id=guild_id, origin="test")
            )
            await delete_guild_handler(msg, ws)

        mock_state.save_file.delete_guild_and_players.assert_called_once()
        # Should send at least the final response
        responses = [s for s in ws.sent if s["type"] == MessageType.DELETE_GUILD.value]
        assert len(responses) == 1

    @pytest.mark.asyncio
    async def test_no_save_file_returns_warning(self, ws):
        from palworld_save_pal.ws.handlers.guild_handler import delete_guild_handler

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.guild_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = DeleteGuildMessage(
                data=DeleteGuildData(guild_id=uuid4(), origin="test")
            )
            await delete_guild_handler(msg, ws)

        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.WARNING.value