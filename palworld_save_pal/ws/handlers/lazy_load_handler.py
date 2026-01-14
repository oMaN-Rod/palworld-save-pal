from fastapi import WebSocket

from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.ws.messages import (
    MessageType,
    RequestPlayerDetailsMessage,
    RequestGuildDetailsMessage,
)
from palworld_save_pal.ws.utils import build_response

logger = create_logger(__name__)


async def ws_callback(ws: WebSocket):
    async def callback(message: str):
        await ws.send_json(build_response(MessageType.PROGRESS_MESSAGE, message))

    return callback


async def request_player_details_handler(
    message: RequestPlayerDetailsMessage, ws: WebSocket
) -> None:
    app_state = get_app_state()
    player_id = message.data

    logger.info(f"Request player details for {player_id}")

    if not app_state.save_file:
        logger.error("No save file loaded")
        await ws.send_json(
            build_response(
                MessageType.GET_PLAYER_DETAILS_RESPONSE,
                {"error": "No save file loaded"},
            )
        )
        return

    async def progress_callback(msg: str):
        await ws.send_json(build_response(MessageType.PROGRESS_MESSAGE, msg))

    player = await app_state.get_player_details(player_id, progress_callback)

    if player:
        await ws.send_json(
            build_response(
                MessageType.GET_PLAYER_DETAILS_RESPONSE,
                {
                    "player": player,
                    "player_id": str(player_id),
                },
            )
        )
        logger.info(f"Sent player details for {player.nickname}")
    else:
        await ws.send_json(
            build_response(
                MessageType.GET_PLAYER_DETAILS_RESPONSE,
                {"error": f"Player {player_id} not found"},
            )
        )


async def request_guild_details_handler(
    message: RequestGuildDetailsMessage, ws: WebSocket
) -> None:
    app_state = get_app_state()
    guild_id = message.data

    logger.info(f"Request guild details for {guild_id}")

    if not app_state.save_file:
        logger.error("No save file loaded")
        await ws.send_json(
            build_response(
                MessageType.GET_GUILD_DETAILS_RESPONSE,
                {"error": "No save file loaded"},
            )
        )
        return

    async def progress_callback(msg: str):
        await ws.send_json(build_response(MessageType.PROGRESS_MESSAGE, msg))

    guild = await app_state.get_guild_details(guild_id, progress_callback)

    if guild:
        await ws.send_json(
            build_response(
                MessageType.GET_GUILD_DETAILS_RESPONSE,
                {
                    "guild": guild,
                    "guild_id": str(guild_id),
                },
            )
        )
        logger.info(f"Sent guild details for {guild.name}")
    else:
        await ws.send_json(
            build_response(
                MessageType.GET_GUILD_DETAILS_RESPONSE,
                {"error": f"Guild {guild_id} not found"},
            )
        )
