from fastapi import WebSocket
from palworld_save_pal.ws.messages import GetUICommonMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)


async def get_ui_common_handler(_: GetUICommonMessage, ws: WebSocket):
    app_state = get_app_state()
    ui_common_i18n_json = JsonManager(
        f"data/json/l10n/{app_state.settings.language}/ui.json"
    )
    ui_common_i18n = ui_common_i18n_json.read()

    response = build_response(MessageType.GET_ACTIVE_SKILLS, ui_common_i18n)
    await ws.send_json(response)
