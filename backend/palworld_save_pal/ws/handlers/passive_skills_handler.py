from fastapi import WebSocket
from palworld_save_pal.state import get_app_state
from palworld_save_pal.ws.messages import GetPassiveSkillsMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)


async def get_passive_skills_handler(_: GetPassiveSkillsMessage, ws: WebSocket):
    app_state = get_app_state()
    passive_skills_json = JsonManager("data/json/passive_skills.json")
    passive_skills_i18n_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/passive_skills.json"
    )
    passive_skills_data = passive_skills_json.read()
    passive_skills_i18n = passive_skills_i18n_json.read()

    localized_data = {}
    for skill_id, details in passive_skills_data.items():
        i18n_info = passive_skills_i18n.get(
            skill_id, {"localized_name": skill_id, "description": ""}
        )
        localized_data[skill_id] = {
            "id": skill_id,
            "localized_name": i18n_info["localized_name"],
            "description": i18n_info["description"],
            "details": {
                **details,
            },
        }

    response = build_response(MessageType.GET_PASSIVE_SKILLS, localized_data)
    await ws.send_json(response)
