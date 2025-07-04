import json
from fastapi import WebSocket
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.ws.messages import MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

friendship_data = JsonManager("data/json/friendship.json").read()

async def get_friendship_data_handler(_: dict, ws: WebSocket):
    response = build_response(MessageType.GET_FRIENDSHIP_DATA, friendship_data)
    await ws.send_json(response)
