import pytest
from unittest.mock import MagicMock, patch

from palworld_save_pal.ws.messages import MessageType


class MockWebSocket:
    def __init__(self):
        self.sent = []

    async def send_json(self, data):
        self.sent.append(data)


class TestGetExpDataHandler:
    @pytest.mark.asyncio
    async def test_sends_exp_data_response(self):
        fake_exp = {"1": 0, "2": 50, "3": 120}

        with patch(
            "palworld_save_pal.ws.handlers.exp_handler.exp_data", fake_exp
        ):
            from palworld_save_pal.ws.handlers.exp_handler import get_exp_data_handler

            ws = MockWebSocket()
            await get_exp_data_handler({}, ws)

            assert len(ws.sent) == 1
            assert ws.sent[0]["type"] == MessageType.GET_EXP_DATA.value
            assert ws.sent[0]["data"] == fake_exp

    @pytest.mark.asyncio
    async def test_response_data_is_dict(self):
        fake_exp = {"1": 0}

        with patch(
            "palworld_save_pal.ws.handlers.exp_handler.exp_data", fake_exp
        ):
            from palworld_save_pal.ws.handlers.exp_handler import get_exp_data_handler

            ws = MockWebSocket()
            await get_exp_data_handler({}, ws)

            assert isinstance(ws.sent[0]["data"], dict)
