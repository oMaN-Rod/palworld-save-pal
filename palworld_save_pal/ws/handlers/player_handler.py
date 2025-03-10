from fastapi import WebSocket
from palworld_save_pal.ws.messages import (
    DeleteGuildMessage,
    DeletePlayerMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)


async def delete_player_handler(message: DeletePlayerMessage, ws: WebSocket):
    player_id = message.data.player_id
    origin = message.data.origin

    app_state = get_app_state()
    save_file = app_state.save_file

    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    success = await save_file.delete_player(
        player_id,
        ws_callback=ws_callback,
    )
    message = {
        "player_id": player_id if success else None,
        "origin": origin,
    }
    response = build_response(MessageType.DELETE_PLAYER, message)
    await ws.send_json(response)
