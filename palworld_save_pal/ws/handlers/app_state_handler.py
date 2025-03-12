from fastapi import WebSocket
import sftpretty
from palworld_save_pal.ws.messages import SetupSFTPConnectionMessage, SyncAppStateMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def sync_app_state_handler(_: SyncAppStateMessage, ws: WebSocket):
    app_state = get_app_state()

    response = build_response(MessageType.GET_SETTINGS, app_state.settings)
    await ws.send_json(response)

    save_file = app_state.save_file
    if save_file is None:
        logger.warning("No save file loaded")
        return

    data = {
        "level": save_file.name,
        "players": [str(p) for p in (save_file.get_players()).keys()],
        "guilds": [str(g) for g in (save_file.get_guilds()).keys()],
        "world_name": save_file.world_name,
        "type": app_state.save_type.name.lower(),
        "size": save_file.size,
    }

    response = build_response(MessageType.LOADED_SAVE_FILES, data)
    await ws.send_json(response)

    response = build_response(MessageType.GET_PLAYERS, save_file.get_players())
    await ws.send_json(response)

    response = build_response(MessageType.GET_GUILDS, save_file.get_guilds())
    await ws.send_json(response)

async def setup_sftp_connection(message: SetupSFTPConnectionMessage, ws: WebSocket):
    app_state = get_app_state()

    hostname = message.data.hostname
    username = message.data.username
    password = message.data.password

    try:
        app_state.sftp_connection = sftpretty.Connection(hostname, username=username, password=password)
    except:
        err_response = build_response(MessageType.SETUP_SFTP_CONNECTION, "An error occured connecting via SFTP. Check your credentials and try again!")
        ws.send_json(err_response)