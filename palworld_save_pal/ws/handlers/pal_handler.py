import copy
import traceback
from fastapi import WebSocket
from fastapi.encoders import jsonable_encoder

from palworld_save_pal.save_file.pal_objects import PalObjects
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.ws.messages import *
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.ws.utils import build_response

logger = create_logger(__name__)

pals_json = JsonManager("data/json/pals.json")
pals_i18n_json = JsonManager("data/json/en-GB/pals.json")


async def get_pals_handler(_: GetPalsMessage, ws: WebSocket):
    try:
        pals_data = pals_json.read()
        pals_i18n = pals_i18n_json.read()

        localized_pals_data = {}
        for code_name, pal_info in pals_data.items():
            localized_pal_info = copy.deepcopy(pal_info)

            if code_name in pals_i18n:
                i18n_data = pals_i18n[code_name]
                localized_pal_info["localized_name"] = i18n_data.get(
                    "localized_name", code_name
                )
                localized_pal_info["description"] = i18n_data.get(
                    "description", "No description available"
                )
            else:
                localized_pal_info["localized_name"] = code_name
                localized_pal_info["description"] = "No description available"

            localized_pals_data[code_name] = localized_pal_info

        response = build_response(MessageType.GET_PALS, localized_pals_data)
        await ws.send_json(response)
    except Exception as e:
        logger.error("Error getting pals: %s", str(e))
        response = build_response(MessageType.ERROR, f"Error getting pals: {str(e)}")
        await ws.send_json(response)


async def add_pal_handler(message: AddPalMessage, ws: WebSocket):
    try:
        player_id = message.data.player_id
        pal_code_name = message.data.pal_code_name
        nickname = message.data.nickname
        container_id = message.data.container_id
        app_state = get_app_state()
        save_file = app_state.save_file
        new_pal = save_file.add_pal(player_id, pal_code_name, nickname, container_id)
        data = {
            "player_id": player_id,
            "pal": new_pal,
        }
        response = build_response(MessageType.ADD_PAL, jsonable_encoder(data))
        await ws.send_json(response)
    except Exception as e:
        logger.error("Error adding pal: %s", str(e))
        response = build_response(MessageType.ERROR, f"Error adding Pal: {str(e)}")
        await ws.send_json(response)
        traceback.print_exc()


async def move_pal_handler(message: MovePalMessage, ws: WebSocket):
    try:
        player_id = message.data.player_id
        pal_id = message.data.pal_id
        container_id = message.data.container_id
        app_state = get_app_state()
        save_file = app_state.save_file
        pal = save_file.move_pal(player_id, pal_id, container_id)
        response = {}
        if isinstance(pal, Pal):
            data = {
                "player_id": str(player_id),
                "pal_id": str(pal.instance_id),
                "container_id": str(container_id),
            }
            response = build_response(MessageType.MOVE_PAL, data)
        else:
            response = build_response(MessageType.WARNING, "Error moving Pal")
        await ws.send_json(response)
    except Exception as e:
        logger.error("Error moving pal: %s", str(e))
        response = build_response(MessageType.ERROR, f"Error moving Pal: {str(e)}")
        await ws.send_json(response)
        traceback.print_exc()


async def clone_pal_handler(message: ClonePalMessage, ws: WebSocket):
    try:
        pal = message.data
        app_state = get_app_state()
        save_file = app_state.save_file
        new_pal = save_file.clone_pal(pal)
        data = {
            "player_id": pal.owner_uid if pal.owner_uid else None,
            "pal": new_pal,
        }
        response = build_response(MessageType.ADD_PAL, jsonable_encoder(data))
        await ws.send_json(response)
    except Exception as e:
        logger.error("Error processing clone_pal: %s", str(e))
        response = build_response(MessageType.ERROR, f"Error cloning Pal: {str(e)}")
        await ws.send_json(response)
        traceback.print_exc()


async def delete_pals_handler(message: DeletePalsMessage, ws: WebSocket):
    try:
        player_id = message.data.player_id
        pal_ids = message.data.pal_ids
        app_state = get_app_state()
        save_file = app_state.save_file
        save_file.delete_pals(player_id, pal_ids)
    except Exception as e:
        logger.error("Error processing delete_pals: %s", str(e))
        response = build_response(
            MessageType.ERROR, f"Error deleting Pal details: {str(e)}"
        )
        await ws.send_json(response)
        traceback.print_exc()


async def heal_pals_handler(message: HealPalsMessage, ws: WebSocket):
    try:
        pal_ids = message.data
        app_state = get_app_state()
        save_file = app_state.save_file
        save_file.heal_pals(pal_ids)
    except Exception as e:
        logger.error("Error processing heal_pals: %s", str(e))
        response = build_response(MessageType.ERROR, f"Error healing Pals: {str(e)}")
        await ws.send_json(response)
        traceback.print_exc()
