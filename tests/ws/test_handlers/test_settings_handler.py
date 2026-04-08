import pytest
from unittest.mock import MagicMock, patch

from palworld_save_pal.dto.settings import SettingsDTO
from palworld_save_pal.ws.messages import MessageType, UpdateSettingsMessage


class MockWebSocket:
    def __init__(self):
        self.sent = []

    async def send_json(self, data):
        self.sent.append(data)


@pytest.fixture
def mock_settings():
    settings = MagicMock()
    settings.language = "en"
    settings.clone_prefix = "C"
    settings.new_pal_prefix = "N"
    settings.debug_mode = False
    settings.cheat_mode = False
    return settings


@pytest.fixture
def mock_app_state(mock_settings):
    state = MagicMock()
    state.settings = mock_settings
    return state


class TestGetSettingsHandler:
    @pytest.mark.asyncio
    async def test_sends_settings_with_correct_type_and_data(self, mock_app_state):
        with patch(
            "palworld_save_pal.ws.handlers.settings_handler.app_state", mock_app_state
        ):
            from palworld_save_pal.ws.handlers.settings_handler import (
                get_settings_handler,
            )

            ws = MockWebSocket()
            await get_settings_handler({}, ws)

            assert len(ws.sent) == 1
            assert ws.sent[0]["type"] == MessageType.GET_SETTINGS.value
            assert ws.sent[0]["data"] is not None


class TestUpdateSettingsHandler:
    @pytest.mark.asyncio
    async def test_updates_settings_and_responds(self, mock_app_state):
        with patch(
            "palworld_save_pal.ws.handlers.settings_handler.app_state", mock_app_state
        ):
            from palworld_save_pal.ws.handlers.settings_handler import (
                update_settings_handler,
            )

            ws = MockWebSocket()
            dto = SettingsDTO(
                language="ja",
                clone_prefix="X",
                new_pal_prefix="Y",
                debug_mode=True,
                cheat_mode=True,
            )
            message = UpdateSettingsMessage(data=dto)
            await update_settings_handler(message, ws)

            mock_app_state.settings.update_from.assert_called_once_with(dto)
            assert len(ws.sent) == 1
            assert ws.sent[0]["type"] == MessageType.GET_SETTINGS.value
