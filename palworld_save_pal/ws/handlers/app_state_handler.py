from fastapi import WebSocket
from palworld_save_pal.ws.messages import SyncAppStateMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def sync_app_state_handler(_: SyncAppStateMessage, ws: WebSocket):
    app_state = get_app_state()
    save_file = app_state.save_file
    if save_file is None:
        logger.warning("No save file loaded")
        return

    if app_state.local:
        data = {
            "sav_file_name": save_file.name,
            "players": [str(p) for p in (save_file.get_players()).keys()],
            "world_name": save_file.world_name,
        }
    else:
        data = {
            "name": save_file.name,
            "size": save_file.size,
            "world_name": save_file.world_name,
        }

    message_type = (
        MessageType.LOADED_SAVE_FILES if app_state.local else MessageType.LOAD_ZIP_FILE
    )
    response = build_response(message_type, data)
    await ws.send_json(response)
    response = build_response(MessageType.GET_PLAYERS, save_file.get_players())
    await ws.send_json(response)
