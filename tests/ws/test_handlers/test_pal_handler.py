"""Tests for pal WS handlers with mocked AppState."""

from unittest.mock import MagicMock, patch
from uuid import uuid4

import pytest

from palworld_save_pal.ws.messages import (
    DeletePalsData,
    DeletePalsMessage,
    HealPalsMessage,
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


@pytest.fixture
def mock_app_state():
    state = MagicMock()
    state.save_file = MagicMock()
    state.settings = MagicMock()
    state.settings.language = "en"
    return state


class TestHealPalsHandler:
    @pytest.mark.asyncio
    async def test_heals_pals(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.pal_handler import heal_pals_handler

        pal_ids = [uuid4(), uuid4()]

        with patch(
            "palworld_save_pal.ws.handlers.pal_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = HealPalsMessage(type="heal_pals", data=pal_ids)
            await heal_pals_handler(msg, ws)

        mock_app_state.save_file.heal_pals.assert_called_once_with(pal_ids)


class TestDeletePalsHandler:
    @pytest.mark.asyncio
    async def test_deletes_player_pals(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.pal_handler import delete_pals_handler

        player_id = uuid4()
        pal_ids = [uuid4()]

        with patch(
            "palworld_save_pal.ws.handlers.pal_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = DeletePalsMessage(
                data=DeletePalsData(player_id=player_id, pal_ids=pal_ids)
            )
            await delete_pals_handler(msg, ws)

        mock_app_state.save_file.delete_player_pals.assert_called_once_with(
            player_id, pal_ids
        )

    @pytest.mark.asyncio
    async def test_deletes_guild_pals(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.pal_handler import delete_pals_handler

        guild_id = uuid4()
        base_id = uuid4()
        pal_ids = [uuid4()]

        with patch(
            "palworld_save_pal.ws.handlers.pal_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = DeletePalsMessage(
                data=DeletePalsData(
                    guild_id=guild_id, base_id=base_id, pal_ids=pal_ids
                )
            )
            await delete_pals_handler(msg, ws)

        mock_app_state.save_file.delete_guild_pals.assert_called_once_with(
            guild_id, base_id, pal_ids
        )


class TestAddPalHandler:
    @pytest.mark.asyncio
    async def test_adds_player_pal(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.pal_handler import add_pal_handler
        from palworld_save_pal.ws.messages import AddPalData, AddPalMessage

        player_id = uuid4()
        container_id = uuid4()
        mock_pal = MagicMock()
        mock_app_state.save_file.add_player_pal.return_value = mock_pal

        with patch(
            "palworld_save_pal.ws.handlers.pal_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = AddPalMessage(
                data=AddPalData(
                    player_id=player_id,
                    character_id="Lambball",
                    nickname="Test",
                    container_id=container_id,
                )
            )
            await add_pal_handler(msg, ws)

        mock_app_state.save_file.add_player_pal.assert_called_once()
        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.ADD_PAL.value


class TestAddPalHandlerEdgeCases:
    @pytest.mark.asyncio
    async def test_no_save_file_returns_warning(self, ws):
        from palworld_save_pal.ws.handlers.pal_handler import add_pal_handler
        from palworld_save_pal.ws.messages import AddPalData, AddPalMessage

        mock_state = MagicMock()
        mock_state.save_file = None

        with patch(
            "palworld_save_pal.ws.handlers.pal_handler.get_app_state",
            return_value=mock_state,
        ):
            msg = AddPalMessage(
                data=AddPalData(character_id="Lambball", nickname="Test")
            )
            await add_pal_handler(msg, ws)

        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.WARNING.value

    @pytest.mark.asyncio
    async def test_no_player_or_guild_returns_warning(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.pal_handler import add_pal_handler
        from palworld_save_pal.ws.messages import AddPalData, AddPalMessage

        with patch(
            "palworld_save_pal.ws.handlers.pal_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = AddPalMessage(
                data=AddPalData(character_id="Lambball", nickname="Test")
            )
            await add_pal_handler(msg, ws)

        assert len(ws.sent) == 1
        assert ws.sent[0]["type"] == MessageType.WARNING.value


class TestHealAllPalsHandler:
    @pytest.mark.asyncio
    async def test_heals_all_player_pals(self, ws, mock_app_state):
        from palworld_save_pal.ws.handlers.pal_handler import heal_all_pals_handler
        from palworld_save_pal.ws.messages import HealAllPalsMessage

        player_id = uuid4()

        with patch(
            "palworld_save_pal.ws.handlers.pal_handler.get_app_state",
            return_value=mock_app_state,
        ):
            msg = HealAllPalsMessage(
                type="heal_all_pals",
                data={"player_id": str(player_id)},
            )
            await heal_all_pals_handler(msg, ws)

        mock_app_state.save_file.heal_all_player_pals.assert_called_once()