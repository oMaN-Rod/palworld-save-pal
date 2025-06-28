# elements_handler.py

from fastapi import WebSocket
from palworld_save_pal.state import get_app_state
from palworld_save_pal.ws.messages import GetBuildingsMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)


async def get_buildings_handler(_: GetBuildingsMessage, ws: WebSocket):
    app_state = get_app_state()
    buildings_json = JsonManager("data/json/buildings.json")
    buildings_l10n_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/buildings.json"
    )
    buildings_data = buildings_json.read()
    buildings_data_l10n = buildings_l10n_json.read()

    localized_data = {}
    for key, value in buildings_data.items():
        i18n_info = buildings_data_l10n.get(key, {"localized_name": key})
        localized_data[key] = {
            "localized_name": i18n_info["localized_name"],
            **value,
        }

    response = build_response(MessageType.GET_BUILDINGS, localized_data)
    await ws.send_json(response)
