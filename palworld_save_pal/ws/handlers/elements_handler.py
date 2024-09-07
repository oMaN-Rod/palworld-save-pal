# elements_handler.py

from fastapi import WebSocket
from palworld_save_pal.ws.messages import GetElementsMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)
elements_json = JsonManager("data/json/elements.json")


async def get_elements_handler(_: GetElementsMessage, ws: WebSocket):
    logger.info("Processing get_elements request")
    try:
        elements_data = elements_json.read()

        response = build_response(MessageType.GET_ELEMENTS, elements_data)
        await ws.send_json(response)
    except Exception as e:
        logger.error("Error getting elements: %s", str(e))
        response = build_response(
            MessageType.ERROR, f"Error getting elements: {str(e)}"
        )
        await ws.send_json(response)
