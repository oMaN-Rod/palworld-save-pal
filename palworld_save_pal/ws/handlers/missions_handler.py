from fastapi import WebSocket
from palworld_save_pal.ws.messages import (
    GetMissionsMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)


async def get_missions_handler(_: GetMissionsMessage, ws: WebSocket):
    app_state = get_app_state()
    missions_json = JsonManager("data/json/missions.json")
    missions_i18n_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/missions.json"
    )
    missions_data = missions_json.read()
    missions_i18n = missions_i18n_json.read()

    localized_data = {}
    for mission_id, details in missions_data.items():
        i18n_info = missions_i18n.get(
            mission_id, {"localized_name": mission_id, "description": ""}
        )
        localized_data[mission_id] = {
            "id": mission_id,
            "localized_name": i18n_info.get("localized_name", mission_id),
            "description": i18n_info.get("description", ""),
            "quest_type": details.get("quest_type", "Main"),
            "rewards": details.get("rewards", {}),
        }

    response = build_response(MessageType.GET_MISSIONS, localized_data)
    await ws.send_json(response)