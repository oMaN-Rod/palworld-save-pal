import traceback
from fastapi import WebSocket
from palworld_save_pal.ws.messages import SyncAppStateMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.logging_config import create_logger
from fastapi.encoders import jsonable_encoder

logger = create_logger(__name__)


async def sync_app_state_handler(_: SyncAppStateMessage, ws: WebSocket):
    app_state = get_app_state()
    save_file = app_state.save_file
    if save_file is None:
        logger.warning("No save file loaded")
        return

    data = {
        "name": save_file.name,
        "size": save_file.size,
    }
    response = build_response(MessageType.LOAD_ZIP_FILE, data)
    await ws.send_json(response)
    data = jsonable_encoder(save_file.get_players())
    response = build_response(MessageType.GET_PLAYERS, data)
    await ws.send_json(response)
