"""Tests for lazy load WS handlers."""

from unittest.mock import AsyncMock, MagicMock, patch
from uuid import uuid4

import pytest

from palworld_save_pal.ws.messages import (
    MessageType,
    RequestPlayerDetailsMessage,
    RequestGuildDetailsMessage,
)


class MockWebSocket:
    def __init__(self):
        self.sent = []

    async def send_json(self, data):
        self.sent.append(data)


@pytest.fixture
def ws():
    return MockWebSocket()


class TestRequestPlayerDetailsHandler:
    @pytest.mark.asyncio
    async def test_no_save_file_returns_error(self, ws):
        from palworld_save_pal.ws.handlers.lazy_load_handler import (
            request_player_details_handler,
        )

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.lazy_load_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = RequestPlayerDetailsMessage(
                type=MessageType.REQUEST_PLAYER_DETAILS.value,
                data={"player_id": str(uuid4()), "origin": "edit"},
            )
            await request_player_details_handler(msg, ws)

        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.GET_PLAYER_DETAILS_RESPONSE.value
        assert "error" in ws.sent[0]["data"]

    @pytest.mark.asyncio
    async def test_player_found(self, ws):
        from palworld_save_pal.ws.handlers.lazy_load_handler import (
            request_player_details_handler,
        )

        mock_player = MagicMock()
        mock_player.nickname = "TestPlayer"
        mock_state = MagicMock()
        mock_state.save_file = MagicMock()
        mock_state.get_player_details = AsyncMock(return_value=mock_player)

        with patch(
            "palworld_save_pal.ws.handlers.lazy_load_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = RequestPlayerDetailsMessage(
                type=MessageType.REQUEST_PLAYER_DETAILS.value,
                data={"player_id": str(uuid4()), "origin": "edit"},
            )
            await request_player_details_handler(msg, ws)

        # Should send at least one response
        responses = [s for s in ws.sent if s["type"] == MessageType.GET_PLAYER_DETAILS_RESPONSE.value]
        assert len(responses) == 1
        assert "player" in responses[0]["data"]

    @pytest.mark.asyncio
    async def test_player_not_found(self, ws):
        from palworld_save_pal.ws.handlers.lazy_load_handler import (
            request_player_details_handler,
        )

        mock_state = MagicMock()
        mock_state.save_file = MagicMock()
        mock_state.get_player_details = AsyncMock(return_value=None)

        with patch(
            "palworld_save_pal.ws.handlers.lazy_load_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = RequestPlayerDetailsMessage(
                type=MessageType.REQUEST_PLAYER_DETAILS.value,
                data={"player_id": str(uuid4()), "origin": "edit"},
            )
            await request_player_details_handler(msg, ws)

        responses = [s for s in ws.sent if s["type"] == MessageType.GET_PLAYER_DETAILS_RESPONSE.value]
        assert len(responses) == 1
        assert "error" in responses[0]["data"]


class TestRequestGuildDetailsHandler:
    @pytest.mark.asyncio
    async def test_no_save_file_returns_error(self, ws):
        from palworld_save_pal.ws.handlers.lazy_load_handler import (
            request_guild_details_handler,
        )

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.lazy_load_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = RequestGuildDetailsMessage(
                type=MessageType.REQUEST_GUILD_DETAILS.value,
                data=str(uuid4()),
            )
            await request_guild_details_handler(msg, ws)

        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.GET_GUILD_DETAILS_RESPONSE.value
        assert "error" in ws.sent[0]["data"]

    @pytest.mark.asyncio
    async def test_guild_found(self, ws):
        from palworld_save_pal.ws.handlers.lazy_load_handler import (
            request_guild_details_handler,
        )

        mock_guild = MagicMock()
        mock_guild.name = "TestGuild"
        mock_state = MagicMock()
        mock_state.save_file = MagicMock()
        mock_state.get_guild_details = AsyncMock(return_value=mock_guild)

        with patch(
            "palworld_save_pal.ws.handlers.lazy_load_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = RequestGuildDetailsMessage(
                type=MessageType.REQUEST_GUILD_DETAILS.value,
                data=str(uuid4()),
            )
            await request_guild_details_handler(msg, ws)

        responses = [s for s in ws.sent if s["type"] == MessageType.GET_GUILD_DETAILS_RESPONSE.value]
        assert len(responses) == 1
        assert "guild" in responses[0]["data"]

    @pytest.mark.asyncio
    async def test_guild_not_found(self, ws):
        from palworld_save_pal.ws.handlers.lazy_load_handler import (
            request_guild_details_handler,
        )

        mock_state = MagicMock()
        mock_state.save_file = MagicMock()
        mock_state.get_guild_details = AsyncMock(return_value=None)

        with patch(
            "palworld_save_pal.ws.handlers.lazy_load_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = RequestGuildDetailsMessage(
                type=MessageType.REQUEST_GUILD_DETAILS.value,
                data=str(uuid4()),
            )
            await request_guild_details_handler(msg, ws)

        responses = [s for s in ws.sent if s["type"] == MessageType.GET_GUILD_DETAILS_RESPONSE.value]
        assert len(responses) == 1
        assert "error" in responses[0]["data"]