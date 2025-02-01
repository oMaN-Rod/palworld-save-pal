import os
import time
import shutil
from typing import Dict
import uuid

from fastapi import WebSocket
from palworld_save_pal.game.save_file import SaveFile, SaveType
from palworld_save_pal.utils.file_manager import FileManager
from palworld_save_pal.utils.gamepass.container_types import (
    Container,
    ContainerFileList,
    ContainerIndex,
)
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
    backup_container_path,
    cleanup_container_path,
    read_container_index,
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


async def save_modded_save_handler(message: SaveModdedSaveMessage, ws: WebSocket):
    logger.debug("Saving modded save file")
    world_name = message.data

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
        await save_modded_gamepass_save(world_name, ws, ws_callback)


async def save_modded_gamepass_save(world_name: str, ws: WebSocket, ws_callback):
    app_state = get_app_state()
    gamepass_save = app_state.selected_gamepass_save
    if not gamepass_save:
        raise ValueError("No GamePass save selected")

    logger.debug("Saving modded GamePass save file: %s", gamepass_save.save_id)

    try:
        container_path = app_state.settings.save_dir

        # Create backup of container path
        await ws_callback("Creating backup of container path...")
        backup_path = backup_container_path(container_path)
        await ws_callback(f"Created backup at: {backup_path}")

        # Read container index and get save containers
        container_index = read_container_index(container_path)
        cleanup_container_path(container_index, container_path)

        original_save_name = gamepass_save.save_id
        original_containers = container_index.get_save_containers(original_save_name)

        if not original_containers:
            raise ValueError(f"No containers found for save: {original_save_name}")

        # Convert current save to SAV format in memory
        await ws_callback("Converting modified save to SAV format...")
        # create a new save_name which consist of a uuid4 all uppercase with no dashes
        new_save_id = uuid.uuid4().hex.upper()
        logger.debug("New save id: %s => %s", gamepass_save.save_id, new_save_id)
        app_state.save_file.name = new_save_id
        save_data = app_state.save_file.sav()

        # Save modified gamepass save with new containers
        await ws_callback("Creating new containers for modified save...")
        save_modified_gamepass(
            container_index=container_index,
            container_path=container_path,
            save_id=new_save_id,
            modified_level_data=save_data,
            original_containers=original_containers,
            world_name=world_name,
        )

        await ws_callback(f"Modded save created as: {new_save_id}")
        response = build_response(
            MessageType.SAVE_MODDED_SAVE,
            f"Created modded save as: {new_save_id}",
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
        "level": validation_result.level_sav,
        "players": [str(p) for p in player_files],
        "world_name": app_state.save_file.world_name,
        "type": "steam",
        "size": app_state.save_file.size,
    }

    response = build_response(MessageType.LOADED_SAVE_FILES, data)
    await ws.send_json(response)

    response = build_response(MessageType.GET_PLAYERS, app_state.players)
    await ws.send_json(response)

    response = build_response(MessageType.GET_GUILDS, app_state.guilds)
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
    container_index: ContainerIndex = read_container_index(app_state.settings.save_dir)
    containers = container_index.get_save_containers(save_id)

    level_sav_container = containers.get("Level", None)
    if level_sav_container is None:
        return
    level_sav_dir = os.path.join(
        app_state.settings.save_dir,
        level_sav_container.container_uuid.bytes_le.hex().upper(),
    )
    seq = 0
    for filename in os.listdir(level_sav_dir):
        if filename.startswith("container."):
            seq = int(filename.split(".")[1])
            logger.debug("Reading container file: %s", filename)
            with open(os.path.join(level_sav_dir, filename), "rb") as f:
                file_list = ContainerFileList.from_stream(f)
                level_sav = file_list.files[0].data

    level_meta_container = containers.get("LevelMeta", None)
    if level_meta_container is None:
        return
    level_meta_dir = os.path.join(
        app_state.settings.save_dir,
        level_meta_container.container_uuid.bytes_le.hex().upper(),
    )
    for filename in os.listdir(level_meta_dir):
        if filename.startswith("container."):
            logger.debug("Reading container file: %s", filename)
            with open(os.path.join(level_meta_dir, filename), "rb") as f:
                file_list = ContainerFileList.from_stream(f)
                level_meta = file_list.files[0].data

    player_containers = [c for k, c in containers.items() if "Player" in k]
    for player_container in player_containers:
        player_dir = os.path.join(
            app_state.settings.save_dir,
            player_container.container_uuid.bytes_le.hex().upper(),
        )
        for filename in os.listdir(player_dir):
            if filename.startswith("container."):
                logger.debug("Reading container file: %s", filename)
                with open(os.path.join(player_dir, filename), "rb") as f:
                    file_list = ContainerFileList.from_stream(f)
                    player_files[
                        uuid.UUID(player_container.container_name.split("-")[-1])
                    ] = file_list.files[0].data

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
    world_name = (
        app_state.save_file.world_name if app_state.save_file.world_name else "Unknown"
    )
    data = {
        "level": f"{level_sav_dir}/container.{seq}",
        "players": list(player_files.keys()),
        "world_name": world_name,
        "type": "gamepass",
        "size": app_state.save_file.size,
    }
    response = build_response(MessageType.LOADED_SAVE_FILES, data)
    await ws.send_json(response)

    response = build_response(MessageType.GET_PLAYERS, app_state.players)
    await ws.send_json(response)

    response = build_response(MessageType.GET_GUILDS, app_state.guilds)
    await ws.send_json(response)
