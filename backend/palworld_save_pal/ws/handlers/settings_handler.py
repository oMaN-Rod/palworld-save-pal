from fastapi import WebSocket
from fastapi.encoders import jsonable_encoder
from palworld_save_pal.editor.settings import Settings
from palworld_save_pal.state import get_app_state
from shared.models import MessageType, UpdateSettingsMessage
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)
app_state = get_app_state()


async def get_settings_handler(_: dict, ws: WebSocket):
    response = build_response(MessageType.GET_SETTINGS, app_state.settings)
    await ws.send_json(response)


async def update_settings_handler(message: UpdateSettingsMessage, ws: WebSocket):
    app_state.settings.update_from(message.data)
    response = build_response(MessageType.GET_SETTINGS, app_state.settings)
    await ws.send_json(response)
