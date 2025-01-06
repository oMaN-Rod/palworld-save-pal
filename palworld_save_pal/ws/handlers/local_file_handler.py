import os
import time
import shutil
import uuid

from fastapi import WebSocket
from palworld_save_pal.game.save_file import SaveFile, SaveType
from palworld_save_pal.utils.file_manager import FileManager
from palworld_save_pal.ws.messages import (
    MessageType,
    SaveModdedSaveMessage,
    SelectGamepassSaveMessage,
    SelectSaveMessage,
)
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.gamepass.container_utils import (
    find_container_path,
    backup_container_path,
    read_container_index,
    get_save_containers,
    save_modified_gamepass,
)

logger = create_logger(__name__)


async def backup_file(file_path: str, save_type: str, ws_callback):
    file_name = os.path.basename(file_path)
    backup_dir = f"backups/{save_type}"
    if not os.path.exists(backup_dir):
        os.makedirs(backup_dir)
    timestamp = time.strftime("%Y-%m-%d-%H-%M")
    extension = ".sav" if save_type == "steam" else ""
    backup_path = os.path.join(backup_dir, f"{file_name}_{timestamp}{extension}")
    await ws_callback(f"Backing up save file {file_path} to {backup_path}...")
    if os.path.exists(file_path):
        shutil.move(file_path, backup_path)
    else:
        await ws_callback(f"Save file {file_path} not found, skipping backup")


async def save_modded_save_handler(_: SaveModdedSaveMessage, ws: WebSocket):
    logger.debug("Saving modded save file")

    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    app_state = get_app_state()
    save_file = app_state.save_file

    if not save_file:
        raise ValueError("No save file loaded")

    if app_state.save_type == SaveType.STEAM:
        await save_modded_steam_save(ws, ws_callback, save_file)
    else:
        await save_modded_gamepass_save(ws, ws_callback)


async def save_modded_gamepass_save(ws: WebSocket, ws_callback):
    app_state = get_app_state()
    gamepass_save = app_state.selected_gamepass_save
    if not gamepass_save:
        raise ValueError("No GamePass save selected")

    logger.debug("Saving modded GamePass save file: %s", gamepass_save.save_id)

    try:
        # Get container path
        container_path = find_container_path()

        # Create backup of container path
        await ws_callback("Creating backup of container path...")
        backup_path = backup_container_path(container_path)
        await ws_callback(f"Created backup at: {backup_path}")

        # Read container index and get save containers
        container_index = read_container_index(container_path)
        original_save_name = gamepass_save.save_id
        # create a new save_name which consist of a uuid4 all uppercase with no dashes
        new_save_name = uuid.uuid4().hex.upper()
        original_containers = get_save_containers(container_index, original_save_name)

        if not original_containers:
            raise ValueError(f"No containers found for save: {original_save_name}")

        # Convert current save to SAV format in memory
        await ws_callback("Converting modified save to SAV format...")
        save_data = app_state.save_file.sav()

        # Save modified gamepass save with new containers
        await ws_callback("Creating new containers for modified save...")
        save_modified_gamepass(
            container_path=container_path,
            save_name=new_save_name,
            modified_level_data=save_data,
            original_containers=original_containers,
        )

        await ws_callback(f"Modded save created as: {new_save_name}")
        response = build_response(
            MessageType.SAVE_MODDED_SAVE,
            f"Created modded save as: {new_save_name}",
        )
        await ws.send_json(response)

    except Exception as e:
        logger.error("Failed to save gamepass save: %s", str(e))
        response = build_response(
            MessageType.ERROR, f"Failed to save gamepass save: {str(e)}"
        )
        await ws.send_json(response)
        raise


async def save_modded_steam_save(ws: WebSocket, ws_callback, save_file: SaveFile):
    await backup_file(save_file.name, "steam", ws_callback)
    save_file.to_sav_file(save_file.name)
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
        await get_gamepass_saves(save_path, ws)


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


async def get_gamepass_saves(file_path: str, ws: WebSocket):
    logger.debug("Getting GamePass saves")
    app_state = get_app_state()
    app_state.gamepass_index_path = file_path
    validation = FileManager.validate_gamepass_directory(file_path)

    if not validation.valid:
        response = build_response(MessageType.ERROR, validation.error)
    else:
        app_state.gamepass_saves = validation.gamepass_saves
        response = build_response(
            MessageType.SELECT_GAMEPASS_SAVE, validation.gamepass_saves
        )

    await ws.send_json(response)


async def select_gamepass_save_handler(
    message: SelectGamepassSaveMessage, ws: WebSocket
):
    save_id = message.data
    logger.debug("Selecting GamePass save: %s", save_id)
    app_state = get_app_state()
    gamepass_save = app_state.select_gamepass_save(save_id)
    logger.debug("Selected GamePass save: %s", gamepass_save)

    level_sav = None
    level_meta = None
    player_files = {}

    for container in gamepass_save.containers:
        logger.debug("Processing container: %s", container.name)
        parts = container.name.split("-")
        if len(parts) < 2:
            continue

        if parts[-2] == "Level" and os.path.exists(container.file):
            with open(container.file, "rb") as f:
                level_sav = f.read()

        if parts[-1] == "Level" and os.path.exists(container.file):
            with open(container.file, "rb") as f:
                level_sav = f.read()

        if parts[-2] == "Players" and os.path.exists(container.file):
            player_id = uuid.UUID(parts[-1])
            with open(container.file, "rb") as f:
                player_files[player_id] = f.read()

        if parts[-1] == "LevelMeta" and os.path.exists(container.file):
            with open(container.file, "rb") as f:
                level_meta = f.read()

    if not level_sav:
        raise ValueError("Level.sav not found in selected save")

    if not player_files or len(player_files.values()) == 0:
        raise ValueError("No player saves found in selected save")

    await app_state.process_save_files(
        save_id,
        level_sav,
        level_meta,
        player_files,
        lambda msg: ws.send_json(build_response(MessageType.PROGRESS_MESSAGE, msg)),
        save_type=SaveType.GAMEPASS,
    )

    response = build_response(
        MessageType.LOADED_SAVE_FILES,
        {"level": save_id, "players": list(player_files.keys())},
    )
    await ws.send_json(response)

    response = build_response(MessageType.GET_PLAYERS, app_state.players)
    await ws.send_json(response)
