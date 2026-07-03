"""delete_pals_handler must re-sync player/guild summaries after deleting."""

import asyncio
from unittest.mock import MagicMock, patch

import pytest

from palworld_save_pal.ws.messages import DeletePalsMessage, MessageType
from tests.game.conftest import PLAYER_O_UID, WORLD1_DIR, _load_save_manager, _noop


class MockWebSocket:
    def __init__(self):
        self.sent = []

    async def send_json(self, data):
        self.sent.append(data)


@pytest.fixture
def fresh_save_manager():
    # Function-scoped: this test mutates the save manager, so it must load
    # its own instance rather than sharing a module/session fixture. Resolved
    # during pytest's sync fixture-setup phase, before the async test's event
    # loop starts running (avoids nesting inside _load_save_manager's loop).
    sm = _load_save_manager(WORLD1_DIR)
    # delete_player_pals requires the player to already be lazy-loaded.
    asyncio.run(sm.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop))
    return sm


@pytest.mark.asyncio
async def test_delete_pushes_summary_resync(fresh_save_manager):
    from palworld_save_pal.ws.handlers.pal_handler import delete_pals_handler

    sm = fresh_save_manager
    state = MagicMock()
    state.save_file = sm

    target = next(p for p in sm.get_pal_summaries() if p.owner_uid == PLAYER_O_UID)
    ws = MockWebSocket()
    msg = DeletePalsMessage(
        type=MessageType.DELETE_PALS.value,
        data={"player_id": str(PLAYER_O_UID), "pal_ids": [str(target.instance_id)]},
    )
    with patch(
        "palworld_save_pal.ws.handlers.pal_handler.get_app_state", return_value=state
    ):
        await delete_pals_handler(msg, ws)

    after_ids = {p.instance_id for p in sm.get_pal_summaries()}
    assert target.instance_id not in after_ids

    sent_types = [item["type"] for item in ws.sent]
    assert MessageType.GET_PLAYER_SUMMARIES.value in sent_types
    assert MessageType.GET_GUILD_SUMMARIES.value in sent_types
