import json
from fastapi import WebSocket
from palworld_save_pal.ws.messages import MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

with open("data/json/exp.json", "r") as f:
    exp_data = json.load(f)


async def get_exp_data_handler(_: dict, ws: WebSocket):
    try:
        response = build_response(MessageType.GET_EXP_DATA, exp_data)
        await ws.send_json(response)
    except Exception as e:
        logger.error("Error getting exp data: %s", str(e))
        response = build_response(
            MessageType.ERROR, f"Error getting exp data: {str(e)}"
        )
        await ws.send_json(response)
