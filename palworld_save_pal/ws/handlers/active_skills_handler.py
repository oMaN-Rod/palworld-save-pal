from fastapi import WebSocket
from palworld_save_pal.ws.messages import GetActiveSkillsMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)
active_skills_json = JsonManager("data/json/active_skills.json")
active_skills_i18n_json = JsonManager("data/json/en-GB/active_skills.json")


async def get_active_skills_handler(_: GetActiveSkillsMessage, ws: WebSocket):
    try:
        active_skills_data = active_skills_json.read()
        active_skills_i18n = active_skills_i18n_json.read()

        combined_active_skills = {}
        for skill_id, details in active_skills_data.items():
            i18n_info = active_skills_i18n.get(
                skill_id, {"name": skill_id, "description": ""}
            )
            combined_active_skills[skill_id] = {
                "id": skill_id,
                "name": i18n_info["name"],
                "description": i18n_info["description"],
                "details": {**details},
            }

        response = build_response(MessageType.GET_ACTIVE_SKILLS, combined_active_skills)
        await ws.send_json(response)
    except Exception as e:
        logger.error("Error getting active skills: %s", str(e))
        response = build_response(
            MessageType.ERROR, f"Error getting active skills: {str(e)}"
        )
        await ws.send_json(response)
