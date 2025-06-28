from fastapi import WebSocket
from palworld_save_pal.ws.messages import (
    DeleteGuildMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)


async def delete_guild_handler(message: DeleteGuildMessage, ws: WebSocket):
    guild_id = message.data.guild_id
    origin = message.data.origin

    app_state = get_app_state()
    save_file = app_state.save_file

    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    await save_file.delete_guild_and_players(
        guild_id,
        ws_callback=ws_callback,
    )
    message = {
        "guild_id": guild_id,
        "origin": origin,
    }
    response = build_response(MessageType.DELETE_GUILD, message)
    await ws.send_json(response)
