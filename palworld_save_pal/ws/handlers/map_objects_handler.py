from fastapi import WebSocket
from palworld_save_pal.ws.messages import (
    GetEffigiesMessage,
    GetFastTravelPointsMessage,
    GetMapObjectsMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)


async def get_map_objects_handler(_: GetMapObjectsMessage, ws: WebSocket):
    response = build_response(
        MessageType.GET_MAP_OBJECTS, JsonManager("data/json/map_objects.json").read()
    )
    await ws.send_json(response)


async def get_fast_travel_points_handler(_: GetFastTravelPointsMessage, ws: WebSocket):
    response = build_response(
        MessageType.GET_FAST_TRAVEL_POINTS,
        JsonManager("data/json/fast_travel_points.json").read(),
    )
    await ws.send_json(response)


async def get_effigies_handler(_: GetEffigiesMessage, ws: WebSocket):
    response = build_response(
        MessageType.GET_EFFIGIES, JsonManager("data/json/effigies.json").read()
    )
    await ws.send_json(response)
