from typing import Any, Dict
from fastapi import WebSocket
from palworld_save_pal.state import get_app_state
from palworld_save_pal.ws.messages import (
    AddGpsPalMessage,
    DeleteGpsPalsMessage,
    RequestGpsMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def request_gps_handler(message: RequestGpsMessage, ws: WebSocket):
    app_state = get_app_state()

    if not app_state.save_file:
        logger.error("No save file loaded")
        await ws.send_json(
            build_response(
                MessageType.GET_GPS_RESPONSE,
                {"error": "No save file loaded"},
            )
        )
        return

    async def progress_callback(msg: str):
        await ws.send_json(build_response(MessageType.PROGRESS_MESSAGE, msg))

    if app_state.gps_loaded and app_state.gps:
        logger.info("GPS already loaded, returning cached data")
        await ws.send_json(build_response(MessageType.GET_GPS_RESPONSE, app_state.gps))
        return

    gps = await app_state.load_gps_on_demand(progress_callback)

    if gps is not None:
        await ws.send_json(build_response(MessageType.GET_GPS_RESPONSE, gps))
        logger.info(f"Sent GPS data with {len(gps)} pals")
    else:
        await ws.send_json(
            build_response(
                MessageType.GET_GPS_RESPONSE,
                {"available": False, "message": "No GPS file available for this save"},
            )
        )


async def add_gps_pal_handler(message: AddGpsPalMessage, ws: WebSocket):
    character_id = message.data.character_id
    nickname = message.data.nickname
    storage_slot = message.data.storage_slot

    app_state = get_app_state()
    save_file = app_state.save_file

    if not app_state.save_file:
        return

    res = save_file.add_gps_pal(character_id, nickname, storage_slot)
    if not res:
        # Failed to add pal, possibly due to no available slots
        response = build_response(
            MessageType.ADD_GPS_PAL,
            {"error": "Failed to add pal. No available slots or invalid data."},
        )
        await ws.send_json(response)
        return
    new_pal, slot_idx = res
    data: Dict[str, Any] = {"pal": new_pal, "index": slot_idx}
    response = build_response(MessageType.ADD_GPS_PAL, data)
    await ws.send_json(response)


async def delete_gps_pals_handler(message: DeleteGpsPalsMessage, _: WebSocket):
    pal_indexes = message.data.pal_indexes
    logger.debug(f"Deleting {len(pal_indexes)} GPS pals")
    app_state = get_app_state()
    save_file = app_state.save_file
    save_file.delete_gps_pals(pal_indexes)
