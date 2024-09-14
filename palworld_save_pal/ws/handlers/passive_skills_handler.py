from fastapi import WebSocket
from palworld_save_pal.ws.messages import GetPassiveSkillsMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)
passive_skills_json = JsonManager("data/json/passive_skills.json")
passive_skills_i18n_json = JsonManager("data/json/en-GB/passive_skills.json")


async def get_passive_skills_handler(_: GetPassiveSkillsMessage, ws: WebSocket):
    try:
        passive_skills_data = passive_skills_json.read()
        passive_skills_i18n = passive_skills_i18n_json.read()

        combined_passive_skills = {}
        for skill_id, details in passive_skills_data.items():
            i18n_info = passive_skills_i18n.get(
                skill_id, {"name": skill_id, "description": ""}
            )
            combined_passive_skills[skill_id] = {
                "id": skill_id,
                "name": i18n_info["name"],
                "description": i18n_info["description"],
                "details": {
                    **details,
                },
            }

        response = build_response(
            MessageType.GET_PASSIVE_SKILLS, combined_passive_skills
        )
        await ws.send_json(response)
    except Exception as e:
        logger.error("Error getting passive skills: %s", str(e))
        response = build_response(
            MessageType.ERROR, f"Error getting passive skills: {str(e)}"
        )
        await ws.send_json(response)
