from fastapi import WebSocket

from palworld_save_pal.ws.messages import (
    AddPresetMessage,
    GetPresetsMessage,
    UpdatePresetMessage,
    DeletePresetMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.db.ctx.presets import (
    get_all_presets,
    add_preset,
    update_preset_name,
    delete_preset,
    populate_presets_from_json,
    nuke_presets,
)

logger = create_logger(__name__)


async def add_preset_handler(message: AddPresetMessage, ws: WebSocket):
    preset = message.data
    preset_id = add_preset(preset.model_dump())

    response = build_response(
        MessageType.ADD_PRESET,
        {"message": "Preset added successfully", "id": preset_id},
    )
    await ws.send_json(response)


async def get_presets_handler(_: GetPresetsMessage, ws: WebSocket):
    populate_presets_from_json()

    presets = get_all_presets()

    response = build_response(MessageType.GET_PRESETS, presets)
    await ws.send_json(response)


async def update_preset_handler(message: UpdatePresetMessage, ws: WebSocket):
    preset_id = message.data.id
    preset_name = message.data.name

    success = update_preset_name(preset_id, preset_name)

    if success:
        response = build_response(
            MessageType.UPDATE_PRESET, f"{preset_name} updated successfully"
        )
    else:
        response = build_response(
            MessageType.ERROR, f"Failed to update preset {preset_id}"
        )

    await ws.send_json(response)


async def delete_presets_handler(message: DeletePresetMessage, ws: WebSocket):
    success = True
    for preset_id in message.data:
        if not delete_preset(preset_id):
            success = False

    if success:
        response = build_response(
            MessageType.DELETE_PRESET, "Presets deleted successfully"
        )
    else:
        response = build_response(
            MessageType.ERROR, "Failed to delete one or more presets"
        )

    await ws.send_json(response)


async def nuke_presets_handler(_: None, ws: WebSocket):
    nuke_presets()
    response = build_response(MessageType.NUKE_PRESETS, "Presets nuked successfully")
    await ws.send_json(response)
