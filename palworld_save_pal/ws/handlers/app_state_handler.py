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

    data = {
        "level": save_file.level_sav_path,
        "players": [str(p) for p in (save_file.get_players()).keys()],
        "guilds": [str(g) for g in (save_file.get_guilds()).keys()],
        "world_name": save_file.world_name,
        "type": app_state.save_type.name.lower(),
        "size": save_file.size,
        "local": app_state.local,
    }

    response = build_response(MessageType.LOADED_SAVE_FILES, data)
    await ws.send_json(response)

    response = build_response(MessageType.GET_PLAYERS, save_file.get_players())
    await ws.send_json(response)

    response = build_response(MessageType.GET_GUILDS, save_file.get_guilds())
    await ws.send_json(response)

    gps = save_file.get_gps()
    if gps:
        response = build_response(MessageType.GET_GPS_PALS, gps)
        await ws.send_json(response)
