import os
import time

from fastapi import WebSocket
from palworld_save_pal.utils.file_manager import FileManager
from palworld_save_pal.ws.messages import (
    MessageType,
    SaveModdedSaveMessage,
    SelectSaveMessage,
)
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)


async def save_modded_save_handler(_: SaveModdedSaveMessage, ws: WebSocket):
    logger.debug("Saving modded save file")

    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    app_state = get_app_state()
    save_file = app_state.save_file

    if not save_file:
        raise ValueError("No save file loaded")

    file_name = os.path.basename(save_file.name)
    backup_dir = "backups"
    if not os.path.exists(backup_dir):
        os.makedirs(backup_dir)
    timestamp = time.strftime("%Y-%m-%d-%M")
    backup_path = os.path.join(backup_dir, f"{file_name}_{timestamp}.sav")
    await ws_callback(f"Backing up save file {save_file.name} to {backup_path}...")
    os.rename(save_file.name, backup_path)
    await ws_callback("Saving modded save file...")
    save_file.to_sav_file(save_file.name)
    await ws_callback(f"Modded save file saved to {save_file.name}")
    response = build_response(
        MessageType.SAVE_MODDED_SAVE, f"Modded save file saved to {save_file.name}"
    )
    await ws.send_json(response)


async def select_save_files_handler(message: SelectSaveMessage, ws: WebSocket):
    logger.debug("Selecting save files")

    save_type = message.data.type
    save_path = message.data.path
    local = message.data.local

    if save_type == "steam":
        await process_steam_save(save_path, ws, local)
    else:
        pass


async def process_steam_save(save_path: str, ws: WebSocket, local: bool):
    logger.debug("Processing Steam save files")
    validation_result = FileManager.validate_steam_save_directory(save_path)
    if not validation_result.valid:
        raise ValueError(validation_result.error)

    logger.debug("Level.sav path: %s", validation_result.level_sav)
    logger.debug("Players directory path: %s", validation_result.players_dir)

    app_state = get_app_state()

    with open(validation_result.level_sav, "rb") as f:
        level_sav = f.read()

    level_meta = None
    if validation_result.level_meta:
        with open(validation_result.level_meta, "rb") as f:
            level_meta = f.read()

    player_files = FileManager.get_player_saves(validation_result.players_dir)

    await app_state.process_save_files(
        save_path,
        level_sav,
        level_meta,
        player_files,
        ws_callback=lambda msg: ws.send_json(
            build_response(MessageType.PROGRESS_MESSAGE, msg)
        ),
        local=local,
    )

    data = {
        "sav_file_name": validation_result.level_sav,
        "players": [str(p) for p in player_files],
        "world_name": app_state.save_file.world_name,
    }

    response = build_response(MessageType.LOADED_SAVE_FILES, data)
    await ws.send_json(response)

    response = build_response(MessageType.GET_PLAYERS, app_state.players)
    await ws.send_json(response)
