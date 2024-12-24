# elements_handler.py

from fastapi import WebSocket
from palworld_save_pal.state import get_app_state
from palworld_save_pal.ws.messages import GetElementsMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)


async def get_elements_handler(_: GetElementsMessage, ws: WebSocket):
    app_state = get_app_state()
    elements_json = JsonManager("data/json/elements.json")
    elements_l10n_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/elements.json"
    )
    elements_data = elements_json.read()
    elements_data_l10n = elements_l10n_json.read()

    localized_data = {}
    for element_id, details in elements_data.items():
        i18n_info = elements_data_l10n.get(element_id, {"localized_name": element_id})
        localized_data[element_id] = {
            "localized_name": i18n_info["localized_name"],
            **details,
        }

    response = build_response(MessageType.GET_ELEMENTS, localized_data)
    await ws.send_json(response)
