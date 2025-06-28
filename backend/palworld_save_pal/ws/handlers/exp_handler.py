import json
from fastapi import WebSocket
from palworld_save_pal.utils.json_manager import JsonManager
from shared.models import MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

exp_data = JsonManager("data/json/exp.json").read()

async def get_exp_data_handler(_: dict, ws: WebSocket):
    response = build_response(MessageType.GET_EXP_DATA, exp_data)
    await ws.send_json(response)
