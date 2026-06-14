import pytest
from unittest.mock import MagicMock

from palworld_save_pal.ws.handlers.steam_id_handler import convert_steam_id_handler
from palworld_save_pal.ws.messages import (
    ConvertSteamIdMessage,
    MessageType,
)


class MockWebSocket:
    def __init__(self):
        self.sent = []

    async def send_json(self, data):
        self.sent.append(data)


def _make_message(steam_input: str) -> ConvertSteamIdMessage:
    return ConvertSteamIdMessage(data={"steam_input": steam_input})


class TestConvertSteamIdHandler:
    @pytest.mark.asyncio
    async def test_numeric_steam_id(self):
        ws = MockWebSocket()
        msg = _make_message("76561198000000000")
        await convert_steam_id_handler(msg, ws)

        assert len(ws.sent) == 1
        resp = ws.sent[0]
        assert resp["type"] == MessageType.CONVERT_STEAM_ID.value
        assert "palworld_uid" in resp["data"]
        assert "nosteam_uid" in resp["data"]
        assert "from_uid" not in resp["data"]

    @pytest.mark.asyncio
    async def test_profile_url(self):
        ws = MockWebSocket()
        msg = _make_message(
            "https://steamcommunity.com/profiles/76561198000000000/"
        )
        await convert_steam_id_handler(msg, ws)

        resp = ws.sent[0]
        assert resp["type"] == MessageType.CONVERT_STEAM_ID.value
        assert "palworld_uid" in resp["data"]

    @pytest.mark.asyncio
    async def test_palworld_uid_hex(self):
        ws = MockWebSocket()
        msg = _make_message("12345678-0000-0000-0000-000000000000")
        await convert_steam_id_handler(msg, ws)

        resp = ws.sent[0]
        assert resp["type"] == MessageType.CONVERT_STEAM_ID.value
        assert resp["data"]["from_uid"] is True
        assert "palworld_uid" in resp["data"]
        assert "nosteam_uid" in resp["data"]

    @pytest.mark.asyncio
    async def test_palworld_uid_no_dashes(self):
        ws = MockWebSocket()
        msg = _make_message("12345678000000000000000000000000")
        await convert_steam_id_handler(msg, ws)

        resp = ws.sent[0]
        assert resp["data"]["from_uid"] is True

    @pytest.mark.asyncio
    async def test_vanity_url_returns_error(self):
        ws = MockWebSocket()
        msg = _make_message("https://steamcommunity.com/id/somename/")
        await convert_steam_id_handler(msg, ws)

        resp = ws.sent[0]
        assert resp["type"] == MessageType.CONVERT_STEAM_ID.value
        assert "error" in resp["data"]
        assert "Vanity" in resp["data"]["error"]

    @pytest.mark.asyncio
    async def test_invalid_input_returns_error(self):
        ws = MockWebSocket()
        msg = _make_message("not_a_valid_input")
        await convert_steam_id_handler(msg, ws)

        resp = ws.sent[0]
        assert "error" in resp["data"]

    @pytest.mark.asyncio
    async def test_steam_prefix_input(self):
        ws = MockWebSocket()
        msg = _make_message("steam_76561198000000000")
        await convert_steam_id_handler(msg, ws)

        resp = ws.sent[0]
        assert "palworld_uid" in resp["data"]

    @pytest.mark.asyncio
    async def test_uid_values_are_uppercase(self):
        ws = MockWebSocket()
        msg = _make_message("76561198000000000")
        await convert_steam_id_handler(msg, ws)

        resp = ws.sent[0]
        assert resp["data"]["palworld_uid"] == resp["data"]["palworld_uid"].upper()
        assert resp["data"]["nosteam_uid"] == resp["data"]["nosteam_uid"].upper()
