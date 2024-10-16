# preset_handler.py
import uuid
from fastapi import WebSocket
from fastapi.encoders import jsonable_encoder
from palworld_save_pal.ws.messages import (
    AddPresetMessage,
    GetPresetsMessage,
    UpdatePresetMessage,
    DeletePresetMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)

presets_json = JsonManager("data/json/presets.json")


async def add_preset_handler(message: AddPresetMessage, ws: WebSocket):
    preset = message.data
    presets_json.append(str(uuid.uuid4()), jsonable_encoder(preset))
    response = build_response(MessageType.ADD_PRESET, "Preset added successfully")
    await ws.send_json(response)


async def get_presets_handler(_: GetPresetsMessage, ws: WebSocket):
    presets = presets_json.read()
    response = build_response(MessageType.GET_PRESETS, presets)
    await ws.send_json(response)


async def update_preset_handler(message: UpdatePresetMessage, ws: WebSocket):
    preset_id = message.data.id
    preset_name = message.data.name
    presets_json.update_name(str(preset_id), preset_name)
    response = build_response(
        MessageType.UPDATE_PRESET, f"{preset_name} updated successfully"
    )
    await ws.send_json(response)


async def delete_presets_handler(message: DeletePresetMessage, ws: WebSocket):
    for preset_id in message.data:
        presets_json.delete(str(preset_id))
    response = build_response(MessageType.DELETE_PRESET, "Preset deleted successfully")
    await ws.send_json(response)
