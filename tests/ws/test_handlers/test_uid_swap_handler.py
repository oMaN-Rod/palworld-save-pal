"""Tests for UID swap WS handler."""

from unittest.mock import AsyncMock, MagicMock, patch
from uuid import uuid4

import pytest

from palworld_save_pal.ws.messages import (
    MessageType,
    SwapPlayerUidsData,
    SwapPlayerUidsMessage,
)


class MockWebSocket:
    def __init__(self):
        self.sent = []

    async def send_json(self, data):
        self.sent.append(data)


@pytest.fixture
def ws():
    return MockWebSocket()


class TestSwapPlayerUidsHandler:
    @pytest.mark.asyncio
    async def test_successful_swap(self, ws):
        from palworld_save_pal.ws.handlers.uid_swap_handler import (
            swap_player_uids_handler,
        )

        old_uid = uuid4()
        new_uid = uuid4()
        mock_state = MagicMock()
        mock_state.save_file = MagicMock()
        mock_state.save_file.swap_player_uids = AsyncMock(
            return_value={"success": True}
        )
        mock_state.save_file.get_player_summaries.return_value = {}
        mock_state.save_file.get_guild_summaries.return_value = {}

        with patch(
            "palworld_save_pal.ws.handlers.uid_swap_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = SwapPlayerUidsMessage(
                data=SwapPlayerUidsData(
                    old_player_uid=old_uid, new_player_uid=new_uid
                )
            )
            await swap_player_uids_handler(msg, ws)

        mock_state.save_file.swap_player_uids.assert_called_once()
        # Should update summaries on success
        mock_state.save_file.get_player_summaries.assert_called_once()
        responses = [s for s in ws.sent if s["type"] == MessageType.SWAP_PLAYER_UIDS.value]
        assert len(responses) == 1

    @pytest.mark.asyncio
    async def test_no_save_file(self, ws):
        from palworld_save_pal.ws.handlers.uid_swap_handler import (
            swap_player_uids_handler,
        )

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.uid_swap_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = SwapPlayerUidsMessage(
                data=SwapPlayerUidsData(
                    old_player_uid=uuid4(), new_player_uid=uuid4()
                )
            )
            await swap_player_uids_handler(msg, ws)

        responses = [s for s in ws.sent if s["type"] == MessageType.SWAP_PLAYER_UIDS.value]
        assert len(responses) == 1
        assert "error" in responses[0]["data"]

    @pytest.mark.asyncio
    async def test_swap_error(self, ws):
        from palworld_save_pal.ws.handlers.uid_swap_handler import (
            swap_player_uids_handler,
        )

        mock_state = MagicMock()
        mock_state.save_file = MagicMock()
        mock_state.save_file.swap_player_uids = AsyncMock(
            return_value={"error": "Both players are the same."}
        )

        with patch(
            "palworld_save_pal.ws.handlers.uid_swap_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = SwapPlayerUidsMessage(
                data=SwapPlayerUidsData(
                    old_player_uid=uuid4(), new_player_uid=uuid4()
                )
            )
            await swap_player_uids_handler(msg, ws)

        responses = [s for s in ws.sent if s["type"] == MessageType.SWAP_PLAYER_UIDS.value]
        assert len(responses) == 1
        # Should NOT update summaries on error
        mock_state.save_file.get_player_summaries.assert_not_called()