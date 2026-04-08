"""Tests for save file WS handlers."""

from unittest.mock import AsyncMock, MagicMock, patch
from uuid import uuid4

import pytest

from palworld_save_pal.ws.messages import (
    MessageType,
    UpdateSaveFileData,
    UpdateSaveFileMessage,
    DownloadSaveFileMessage,
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
    state.save_file.update_pals = AsyncMock()
    state.save_file.update_players = AsyncMock()
    state.save_file.update_guilds = AsyncMock()
    state.save_file.update_dps_pals = AsyncMock()
    state.save_file.update_gps_pals = AsyncMock()
    state.save_file.get_players.return_value = {}
    return state


class TestUpdateSaveFileHandler:
    @pytest.mark.asyncio
    async def test_no_modifications(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.save_file_handler import (
            update_save_file_handler,
        )

        with patch(
            "palworld_save_pal.ws.handlers.save_file_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = UpdateSaveFileMessage(data=UpdateSaveFileData())
            await update_save_file_handler(msg, ws)

        # No update methods should be called
        mock_app_state.save_file.update_pals.assert_not_called()
        mock_app_state.save_file.update_players.assert_not_called()
        # Should still send the final response
        responses = [
            s for s in ws.sent if s["type"] == MessageType.UPDATE_SAVE_FILE.value
        ]
        assert len(responses) == 1

    @pytest.mark.asyncio
    async def test_no_save_file_raises(self, ws):
        from palworld_save_pal.ws.handlers.save_file_handler import (
            update_save_file_handler,
        )

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.save_file_handler.get_app_state",
            return_value=mock_state,
        ):
            with pytest.raises(ValueError, match="No save file loaded"):
                msg = UpdateSaveFileMessage(data=UpdateSaveFileData())
                await update_save_file_handler(msg, ws)


class TestDownloadSaveFileHandler:
    @pytest.mark.asyncio
    async def test_no_save_file_raises(self, ws):
        from palworld_save_pal.ws.handlers.save_file_handler import (
            download_save_file_handler,
        )

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.save_file_handler.get_app_state",
            return_value=mock_state,
        ):
            with pytest.raises(ValueError, match="No save file loaded"):
                msg = DownloadSaveFileMessage(
                    type=MessageType.DOWNLOAD_SAVE_FILE.value
                )
                await download_save_file_handler(msg, ws)