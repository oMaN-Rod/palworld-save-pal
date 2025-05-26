from fastapi import WebSocket
from palworld_save_pal.ws.messages import GetWorkSuitabilityMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)


async def get_work_suitability_handler(_: GetWorkSuitabilityMessage, ws: WebSocket):
    app_state = get_app_state()
    work_suitability_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/work_suitability.json"
    )
    response = build_response(
        MessageType.GET_WORK_SUITABILITY, work_suitability_json.read()
    )
    await ws.send_json(response)
