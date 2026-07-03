"""Tests for the get_pal_summaries WS handler."""

import asyncio
from unittest.mock import MagicMock, patch

import pytest

from palworld_save_pal.ws.messages import GetPalSummariesMessage, MessageType
from tests.game.conftest import WORLD1_DIR, _load_save_manager


class MockWebSocket:
    def __init__(self):
        self.sent = []

    async def send_json(self, data):
        self.sent.append(data)


@pytest.fixture
def ws():
    return MockWebSocket()


@pytest.mark.asyncio
async def test_no_save_file_returns_error(ws):
    from palworld_save_pal.ws.handlers.pal_handler import get_pal_summaries_handler

    state = MagicMock()
    state.save_file = None
    with patch(
        "palworld_save_pal.ws.handlers.pal_handler.get_app_state", return_value=state
    ):
        await get_pal_summaries_handler(GetPalSummariesMessage(), ws)

    assert ws.sent[0]["type"] == MessageType.GET_PAL_SUMMARIES.value
    assert "error" in ws.sent[0]["data"]


def test_returns_all_pal_summaries(ws):
    from palworld_save_pal.ws.handlers.pal_handler import get_pal_summaries_handler

    state = MagicMock()
    state.save_file = _load_save_manager(WORLD1_DIR)
    with patch(
        "palworld_save_pal.ws.handlers.pal_handler.get_app_state", return_value=state
    ):
        asyncio.run(get_pal_summaries_handler(GetPalSummariesMessage(), ws))

    data = ws.sent[0]["data"]
    assert len(data["pals"]) == len(state.save_file.get_pal_summaries())
    assert len(data["pals"]) > 0
    first = data["pals"][0]
    assert "instance_id" in first
    assert "character_key" in first
