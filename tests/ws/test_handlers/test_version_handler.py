import pytest

from palworld_save_pal.__version__ import __version__
from palworld_save_pal.ws.handlers.version_handler import get_version_handler
from palworld_save_pal.ws.messages import MessageType


class MockWebSocket:
    def __init__(self):
        self.sent = []

    async def send_json(self, data):
        self.sent.append(data)


class TestGetVersionHandler:
    @pytest.mark.asyncio
    async def test_sends_version_response(self):
        ws = MockWebSocket()
        await get_version_handler({}, ws)

        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.GET_VERSION.value

    @pytest.mark.asyncio
    async def test_response_contains_version_string(self):
        ws = MockWebSocket()
        await get_version_handler({}, ws)

        assert ws.sent[0]["data"] == __version__
