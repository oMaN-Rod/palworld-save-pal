import traceback
from uuid import UUID

from fastapi import WebSocket

from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.ws.messages import GetPalDetailsMessage, MessageType
from palworld_save_pal.ws.utils import build_response

logger = create_logger(__name__)


async def get_pal_details_handler(message: GetPalDetailsMessage, ws: WebSocket):
    logger.info("Processing get_pal_details request: %s", message)
    try:
        if isinstance(message.data, str):
            pal_id = UUID(message.data)
        elif isinstance(message.data, UUID):
            pal_id = message.data
        else:
            raise ValueError(f"Invalid Pal ID: {message.data}")

        app_state = get_app_state()
        save_file = app_state.save_file

        if not save_file:
            raise ValueError("No save file loaded")

        pal = save_file.get_pal(pal_id)
        if not pal:
            raise ValueError(f"No Pal found with ID {pal_id}")

        data = pal.model_dump_json()
        response = build_response(MessageType.GET_PAL_DETAILS, data)
        await ws.send_json(response)

    except Exception as e:
        logger.error("Error processing get_pal_details: %s", str(e))
        response = build_response(
            MessageType.ERROR, f"Error getting Pal details: {str(e)}"
        )
        await ws.send_json(response)
        traceback.print_exc()
