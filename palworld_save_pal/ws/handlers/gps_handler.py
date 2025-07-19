from typing import Any, Dict
from fastapi import WebSocket
from palworld_save_pal.state import get_app_state
from palworld_save_pal.ws.messages import (
    AddGpsPalMessage,
    DeleteGpsPalsMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


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
