from fastapi import WebSocket
from palworld_save_pal.ws.messages import GetActiveSkillsMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)


async def get_active_skills_handler(_: GetActiveSkillsMessage, ws: WebSocket):
    app_state = get_app_state()
    active_skills_json = JsonManager("data/json/active_skills.json")
    active_skills_i18n_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/active_skills.json"
    )
    active_skills_data = active_skills_json.read()
    active_skills_i18n = active_skills_i18n_json.read()

    localized_data = {}
    for skill_id, details in active_skills_data.items():
        i18n_info = active_skills_i18n.get(
            skill_id, {"localized_name": skill_id, "description": ""}
        )
        localized_data[skill_id] = {
            "id": skill_id,
            "localized_name": i18n_info["localized_name"],
            "description": i18n_info["description"],
            "details": {**details},
        }

    response = build_response(MessageType.GET_ACTIVE_SKILLS, localized_data)
    await ws.send_json(response)
