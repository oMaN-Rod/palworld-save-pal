import base64
import traceback
from fastapi import WebSocket
from fastapi.encoders import jsonable_encoder
from palworld_save_pal.ws.messages import (
    DownloadSaveFileMessage,
    LoadSaveFileMessage,
    MessageType,
    UpdateSaveFileMessage,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def load_save_file_handler(message: LoadSaveFileMessage, ws: WebSocket):
    logger.info("Processing save file upload")

    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    try:
        app_state = get_app_state()
        file_data = bytes(message.data)
        await app_state.process_save_file(file_data, ws_callback)
        data = {
            "name": app_state.save_file.name,
            "size": app_state.save_file.size,
        }
        logger.debug("Save file loaded: %s", app_state.save_file.name)
        await ws_callback(
            "File uploaded and processed successfully, results coming right up!"
        )
        response = build_response(
            MessageType.LOAD_SAVE_FILE, data, "File uploaded and processed successfully"
        )
        await ws.send_json(response)
        # data = {f"{k}": v.model_dump() for k, v in app_state.players.items()}
        data = jsonable_encoder(app_state.players)
        response = build_response(MessageType.GET_PLAYERS, data)
        await ws.send_json(response)

    except Exception as e:
        logger.error("Error processing save file: %s", str(e))
        response = build_response(
            MessageType.ERROR, None, f"Error processing file: {str(e)}"
        )
        traceback.print_exc()
        await ws.send_json(response)


async def update_save_file_handler(message: UpdateSaveFileMessage, ws: WebSocket):
    logger.info("Processing save file update")

    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    try:
        modified_pals = message.data
        logger.debug("Modified pals: %s", modified_pals)

        app_state = get_app_state()
        save_file = app_state.save_file

        if not save_file:
            raise ValueError("No save file loaded")

        await save_file.update_pals(modified_pals, ws_callback)
        response = build_response(MessageType.UPDATE_SAVE_FILE, "Changes saved")
        await ws.send_json(response)
    except Exception as e:
        logger.error("Error processing save file update: %s", str(e))
        response = build_response(
            MessageType.ERROR, None, f"Error processing changes: {str(e)}"
        )
        traceback.print_exc()
        await ws.send_json(response)


async def download_save_file_handler(_: DownloadSaveFileMessage, ws: WebSocket):
    logger.info("Processing save file download")

    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    try:
        app_state = get_app_state()
        save_file = app_state.save_file

        if not save_file:
            raise ValueError("No save file loaded")
        await ws_callback("Compressing GVAS to sav 💪...")
        sav_file = save_file.sav()
        ws_callback("Encoding sav file to base64 🤖, get ready here it comes...")
        encoded_data = base64.b64encode(sav_file).decode("utf-8")
        data = {
            "name": save_file.name,
            "content": encoded_data,
        }
        logger.debug("Generated save file and sending to client")
        response = build_response(MessageType.DOWNLOAD_SAVE_FILE, data)
        await ws.send_json(response)

    except Exception as e:
        logger.error("Error processing save file download: %s", str(e))
        response = build_response(
            MessageType.ERROR, None, f"Error downloading file: {str(e)}"
        )
        traceback.print_exc()
        await ws.send_json(response)
