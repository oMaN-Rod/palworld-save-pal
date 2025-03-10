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
    
    app_state = get_app_state()
    save_file = app_state.save_file

    data = save_file.delete_guild_and_players(guild_id)
    response = build_response(MessageType.DELETE_GUILD, data)
    await ws.send_json(response)
