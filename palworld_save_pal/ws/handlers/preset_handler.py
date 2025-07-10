import json

import webview
from fastapi import WebSocket

from palworld_save_pal.ws.messages import (
    AddPresetMessage,
    GetPresetsMessage,
    UpdatePresetMessage,
    DeletePresetMessage,
    ExportPresetMessage,
    ImportPresetMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.state import get_app_state
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


async def export_preset_handler(message: ExportPresetMessage, ws: WebSocket):
    """Handle preset export - opens file dialog and saves preset as JSON"""
    try:
        app_state = get_app_state()
        preset_id = message.data.preset_id
        preset_type = message.data.preset_type
        preset_name = message.data.preset_name

        # Get the preset data
        presets = get_all_presets()
        if preset_id not in presets:
            response = build_response(
                MessageType.ERROR, f"Preset {preset_id} not found"
            )
            await ws.send_json(response)
            return

        preset_data = presets[preset_id]

        # Suggest a filename in the format: {preset_type}_{preset_name}.json
        safe_name = "".join(
            c for c in preset_name if c.isalnum() or c in (" ", "-", "_")
        ).rstrip()
        suggested_filename = f"{preset_type}_{safe_name}.json"

        # Open save dialog
        if hasattr(app_state, "webview_window") and app_state.webview_window:
            file_path = app_state.webview_window.create_file_dialog(
                dialog_type=webview.SAVE_DIALOG,
                file_types=("JSON Files (*.json)", "All files (*.*)"),
                save_filename=suggested_filename,
            )

            if file_path and len(file_path) > 0:
                # Handle the file path properly - webview returns a tuple/list
                if isinstance(file_path, (list, tuple)):
                    save_path = str(file_path[0])
                else:
                    save_path = str(file_path)

                # Clean up any extra quotes or formatting
                save_path = save_path.strip().strip("'\"")

                # Save the preset data to JSON file
                with open(save_path, "w", encoding="utf-8") as f:
                    json.dump(preset_data, f, indent=2, ensure_ascii=False)

                response = build_response(
                    MessageType.EXPORT_PRESET,
                    {
                        "message": f"Preset exported successfully to {save_path}",
                        "file_path": save_path,
                    },
                )
            else:
                response = build_response(
                    MessageType.WARNING, "Export cancelled by user"
                )
        else:
            response = build_response(MessageType.ERROR, "File dialog not available")

    except Exception as e:
        logger.error(f"Error exporting preset: {e}")
        response = build_response(
            MessageType.ERROR, f"Failed to export preset: {str(e)}"
        )

    await ws.send_json(response)


async def import_preset_handler(message: ImportPresetMessage, ws: WebSocket):
    """Handle preset import - opens file dialog and loads preset from JSON"""
    try:
        app_state = get_app_state()

        # Open file dialog
        if hasattr(app_state, "webview_window") and app_state.webview_window:
            file_path = app_state.webview_window.create_file_dialog(
                dialog_type=webview.OPEN_DIALOG,
                file_types=("JSON Files (*.json)", "All files (*.*)"),
                allow_multiple=False,
            )

            if file_path and len(file_path) > 0:
                # Handle the file path properly - webview returns a tuple/list
                if isinstance(file_path, (list, tuple)):
                    load_path = str(file_path[0])
                else:
                    load_path = str(file_path)

                # Clean up any extra quotes or formatting
                load_path = load_path.strip().strip("'\"")

                logger.debug(f"Importing preset from {load_path}")
                # Load and validate the preset data
                with open(load_path, "r", encoding="utf-8") as f:
                    preset_data = json.load(f)

                # Validate that it's a valid preset structure
                required_fields = ["name", "type"]
                if not all(field in preset_data for field in required_fields):
                    response = build_response(
                        MessageType.ERROR,
                        "Invalid preset file: missing required fields (name, type)",
                    )
                    await ws.send_json(response)
                    return

                # Remove all ID fields to avoid conflicts (will be auto-generated)
                if "id" in preset_data:
                    preset_data.pop("id")
                if "pal_preset_id" in preset_data:
                    preset_data.pop("pal_preset_id")

                # Remove IDs from nested objects
                if (
                    "pal_preset" in preset_data
                    and preset_data["pal_preset"]
                    and "id" in preset_data["pal_preset"]
                ):
                    preset_data["pal_preset"].pop("id")

                # Remove IDs from other container types if they exist
                for container_key in [
                    "common_container",
                    "weapon_load_out_container",
                    "player_equipment_armor_container",
                    "storage_container",
                    "essential_container",
                    "food_equip_container",
                ]:
                    if (
                        container_key in preset_data
                        and preset_data[container_key]
                        and "id" in preset_data[container_key]
                    ):
                        preset_data[container_key].pop("id")

                # Add the preset
                preset_id = add_preset(preset_data)

                response = build_response(
                    MessageType.IMPORT_PRESET,
                    {
                        "message": f"Preset '{preset_data['name']}' imported successfully",
                        "preset_id": preset_id,
                        "file_path": load_path,
                    },
                )
            else:
                response = build_response(
                    MessageType.WARNING, "Import cancelled by user"
                )
        else:
            response = build_response(MessageType.ERROR, "File dialog not available")

    except json.JSONDecodeError as e:
        logger.error(f"Invalid JSON in preset file: {e}")
        response = build_response(MessageType.ERROR, f"Invalid JSON file: {str(e)}")
    except Exception as e:
        logger.error(f"Error importing preset: {e}")
        response = build_response(
            MessageType.ERROR, f"Failed to import preset: {str(e)}"
        )

    await ws.send_json(response)
