from fastapi import WebSocket
from palworld_save_pal.ws.messages import SyncAppStateMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def sync_app_state_handler(_: SyncAppStateMessage, ws: WebSocket):
    app_state = get_app_state()

    response = build_response(MessageType.GET_SETTINGS, app_state.settings)
    await ws.send_json(response)

    save_file = app_state.save_file
    if save_file is None:
        logger.warning("No save file loaded")
        return

    # Use summaries for player/guild lists
    player_ids = [str(p) for p in app_state.player_summaries.keys()]
    guild_ids = [str(g) for g in app_state.guild_summaries.keys()]

    data = {
        "level": save_file.level_sav_path,
        "players": player_ids,
        "guilds": guild_ids,
        "world_name": save_file.world_name,
        "type": app_state.save_type.name.lower(),
        "size": save_file.size,
        "has_gps": app_state.has_gps_available(),
    }

    response = build_response(MessageType.LOADED_SAVE_FILES, data)
    await ws.send_json(response)

    # Send lightweight summaries - players/guilds loaded on-demand
    response = build_response(
        MessageType.GET_PLAYER_SUMMARIES, app_state.player_summaries
    )
    await ws.send_json(response)

    response = build_response(
        MessageType.GET_GUILD_SUMMARIES, app_state.guild_summaries
    )
    await ws.send_json(response)
