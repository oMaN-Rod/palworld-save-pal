from fastapi import WebSocket
from palworld_save_pal.state import get_app_state
from palworld_save_pal.ws.messages import (
    GetLabResearchMessage,
    UpdateLabResearchMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)
app_state = get_app_state()


async def get_lab_research_handler(_: GetLabResearchMessage, ws: WebSocket):
    base_json = JsonManager("data/json/lab_research.json").read()
    l10n_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/lab_research.json"
    ).read()

    localized_data = {}
    for research_id, details in base_json.items():
        i18n_info = l10n_json.get(
            research_id, {"localized_name": research_id, "description": None}
        )
        localized_data[research_id] = {
            "id": research_id,
            "localized_name": i18n_info["localized_name"],
            "description": i18n_info["description"],
            "details": details,
        }

    response = build_response(MessageType.GET_LAB_RESEARCH, localized_data)
    await ws.send_json(response)


async def update_lab_research_handler(message: UpdateLabResearchMessage, ws: WebSocket):
    guild_id = message.data.guild_id
    research_updates = message.data.research_updates
    save_file = app_state.save_file
    if not save_file:
        logger.error("No save file loaded, cannot update lab research.")
        await ws.send_json(build_response(MessageType.ERROR, "No save file loaded."))
        return

    guild = save_file.get_guild(guild_id)
    if not guild:
        logger.error(f"Guild {guild_id} not found, cannot update lab research.")
        await ws.send_json(
            build_response(MessageType.ERROR, f"Guild {guild_id} not found.")
        )
        return

    try:
        guild.update_lab_research(research_updates)
        response = build_response(
            MessageType.UPDATE_LAB_RESEARCH,
            {"success": True, "guild_id": str(guild_id)},
        )
        await ws.send_json(response)
    except Exception as e:
        logger.exception(f"Error updating lab research for guild {guild_id}: {e}")
        await ws.send_json(
            build_response(MessageType.ERROR, f"Failed to update lab research: {e}")
        )
