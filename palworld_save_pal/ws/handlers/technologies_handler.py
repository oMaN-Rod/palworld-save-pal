from fastapi import WebSocket
from palworld_save_pal.ws.messages import GetTechnologiesMessage, MessageType, SetTechnologyDataMessage
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)


async def get_technologies_handler(_: GetTechnologiesMessage, ws: WebSocket):
    app_state = get_app_state()
    technologies_json = JsonManager("data/json/technologies.json")
    technologies_i18n_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/technologies.json"
    )
    technologies_data = technologies_json.read()
    technologies_i18n = technologies_i18n_json.read()

    print(technologies_data)

    localized_data = {}
    for tech_id, details in technologies_data.items():
        i18n_info = technologies_i18n.get(
            tech_id, {"localized_name": tech_id, "description": ""}
        )
        localized_data[tech_id] = {
            "id": tech_id,
            "localized_name": i18n_info["localized_name"],
            "description": i18n_info["description"],
            "details": {**details},
        }

    response = build_response(MessageType.GET_TECHNOLOGIES, localized_data)
    await ws.send_json(response)

async def set_technology_data_handler(message: SetTechnologyDataMessage, ws: WebSocket):
    app_state = get_app_state()
    save_file = app_state.save_file

    if save_file is None:
        logger.error("No save file loaded")
        return
    
    save_file.update_player_technologies(
        message.data.playerID,
        message.data.technologies,
        message.data.techPoints,
        message.data.ancientTechPoints
    )
    
    response = build_response(MessageType.SET_TECHNOLOGY_DATA, {"success": True})
    await ws.send_json(response)