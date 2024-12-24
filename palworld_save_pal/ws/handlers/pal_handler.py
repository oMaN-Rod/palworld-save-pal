import copy
from fastapi import WebSocket

from palworld_save_pal.game.pal import Pal
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.ws.messages import (
    GetPalsMessage,
    AddPalMessage,
    MovePalMessage,
    ClonePalMessage,
    DeletePalsMessage,
    HealPalsMessage,
    MessageType,
)
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.ws.utils import build_response

logger = create_logger(__name__)


async def get_pals_handler(_: GetPalsMessage, ws: WebSocket):
    app_state = get_app_state()
    pals_json = JsonManager("data/json/pals.json")
    pals_i18n_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/pals.json"
    )
    pals_data = pals_json.read()
    pals_i18n = pals_i18n_json.read()

    localized_data = {}
    for code_name, pal_info in pals_data.items():
        if code_name in pals_i18n:
            i18n_data = pals_i18n[code_name]
            pal_info["localized_name"] = i18n_data.get("localized_name", code_name)
            pal_info["description"] = i18n_data.get(
                "description", "No description available"
            )
        else:
            pal_info["localized_name"] = code_name
            pal_info["description"] = "No description available"

        localized_data[code_name] = pal_info

    response = build_response(MessageType.GET_PALS, localized_data)
    await ws.send_json(response)


async def add_pal_handler(message: AddPalMessage, ws: WebSocket):
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
    response = build_response(MessageType.ADD_PAL, data)
    await ws.send_json(response)


async def move_pal_handler(message: MovePalMessage, ws: WebSocket):
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
            "slot_index": pal.storage_slot,
        }
        response = build_response(MessageType.MOVE_PAL, data)
    else:
        response = build_response(MessageType.WARNING, "Pal container is full")
    await ws.send_json(response)


async def clone_pal_handler(message: ClonePalMessage, ws: WebSocket):
    pal = message.data
    app_state = get_app_state()
    save_file = app_state.save_file
    new_pal = save_file.clone_pal(pal)
    data = {
        "player_id": pal.owner_uid if pal.owner_uid else None,
        "pal": new_pal,
    }
    response = build_response(MessageType.ADD_PAL, data)
    await ws.send_json(response)


async def delete_pals_handler(message: DeletePalsMessage, _: WebSocket):
    player_id = message.data.player_id
    pal_ids = message.data.pal_ids
    app_state = get_app_state()
    save_file = app_state.save_file
    save_file.delete_pals(player_id, pal_ids)


async def heal_pals_handler(message: HealPalsMessage, _: WebSocket):
    pal_ids = message.data
    app_state = get_app_state()
    save_file = app_state.save_file
    save_file.heal_pals(pal_ids)
