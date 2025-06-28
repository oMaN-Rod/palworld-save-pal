from fastapi import WebSocket

from palworld_save_pal.state import get_app_state
from palworld_save_pal.ws.messages import GetItemsMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)


async def get_items_handler(_: GetItemsMessage, ws: WebSocket):
    app_state = get_app_state()
    items_json = JsonManager("data/json/items.json")
    items_i18n_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/items.json"
    )
    items_data = items_json.read()
    items_i18n = items_i18n_json.read()

    localized_data = {}
    for item_id, details in items_data.items():
        i18n_info = items_i18n.get(
            item_id, {"localized_name": item_id, "description": ""}
        )
        localized_data[item_id] = {
            "id": item_id,
            "details": details,
            "info": i18n_info,
        }

    response = build_response(MessageType.GET_ITEMS, localized_data)
    await ws.send_json(response)
