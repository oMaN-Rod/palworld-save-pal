import os
import uuid
from pathlib import Path

import webview
from fastapi import WebSocket

from palworld_save_pal.game.gvas_codec import SaveType
from palworld_save_tools.palsav import decompress_sav_to_gvas, compress_gvas_to_sav
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.file_manager import FileManager
from palworld_save_pal.utils.gamepass.container_types import ContainerFileList
from palworld_save_pal.utils.gamepass.container_utils import (
    backup_container_path,
    create_new_container,
    find_container_path,
    read_container_index,
)
from palworld_save_pal.utils.logging_config import create_logger
import shutil

from palworld_save_pal.game.save_manager import SaveManager
from palworld_save_pal.ws.messages import (
    BaseMessage,
    ConvertSaveFormatMessage,
    DeleteGamepassPlayerMessage,
    DeleteGamepassSaveMessage,
    MessageType,
    RenameGamepassWorldMessage,
)
from palworld_save_pal.ws.utils import build_response

logger = create_logger(__name__)

STEAM_COMPRESSION = 0x31  # PLM / Oodle


def recompress_to_steam(data: bytes) -> bytes:
    """Recompress a .sav file to Steam format (PLM/Oodle) if needed."""
    # Check if already PLM format
    if len(data) > 12 and data[8:12] == b"PlM1":
        return data
    raw_gvas, _ = decompress_sav_to_gvas(data)
    return compress_gvas_to_sav(raw_gvas, STEAM_COMPRESSION)


async def scan_gamepass_saves_handler(message: BaseMessage, ws: WebSocket):
    """Scan GamePass installation and return available saves."""
    try:
        container_path = find_container_path()
    except Exception:
        # No GamePass installation found — return empty saves, not an error
        response = build_response(
            MessageType.SCAN_GAMEPASS_SAVES,
            {"saves": {}, "container_path": None},
        )
        await ws.send_json(response)
        return

    logger.info("Scanning GamePass saves from: %s", container_path)
    saves = FileManager.parse_gamepass_saves(Path(container_path))
    logger.info("Found %d GamePass saves", len(saves))
    saves_dict = {k: v.model_dump() for k, v in saves.items()}
    response = build_response(
        MessageType.SCAN_GAMEPASS_SAVES,
        {"saves": saves_dict, "container_path": container_path},
    )
    await ws.send_json(response)


async def convert_save_format_handler(message: ConvertSaveFormatMessage, ws: WebSocket):
    target_format = message.data.target_format
    source_path = message.data.source_path
    output_path = message.data.output_path
    save_id = message.data.save_id

    async def ws_callback(msg: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, msg)
        await ws.send_json(response)

    app_state = get_app_state()

    if save_id and target_format == "steam":
        # GamePass→Steam with specific save selected via browser
        await _convert_gamepass_save_to_steam(save_id, ws, ws_callback)
    elif source_path == "__select__" or output_path == "__select__":
        # Standalone conversion with file dialog prompts
        resolved_source, resolved_output = _prompt_standalone_paths(
            app_state, target_format
        )
        if not resolved_source or not resolved_output:
            response = build_response(
                MessageType.CONVERT_SAVE_FORMAT,
                {"error": "No file selected."},
            )
            await ws.send_json(response)
            return
        await _convert_standalone(
            resolved_source, resolved_output, target_format, ws, ws_callback
        )
    elif source_path and output_path:
        # Standalone conversion with explicit paths
        await _convert_standalone(
            source_path, output_path, target_format, ws, ws_callback
        )
    elif app_state.save_file:
        # Convert currently loaded save
        await _convert_loaded_save(target_format, ws, ws_callback)
    else:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": "No save file loaded and no source path provided."},
        )
        await ws.send_json(response)


async def _convert_loaded_save(target_format: str, ws: WebSocket, ws_callback):
    app_state = get_app_state()

    if target_format == "gamepass":
        await _loaded_save_to_gamepass(app_state, ws, ws_callback)
    elif target_format == "steam":
        await _loaded_save_to_steam(app_state, ws, ws_callback)
    else:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": f"Unknown target format: {target_format}"},
        )
        await ws.send_json(response)


async def _convert_gamepass_save_to_steam(save_id: str, ws: WebSocket, ws_callback):
    """Convert a specific GamePass save to Steam format, prompting for output dir."""
    app_state = get_app_state()
    window = app_state.webview_window
    if not window:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": "Desktop mode required."},
        )
        await ws.send_json(response)
        return

    # Prompt for output directory
    result = window.create_file_dialog(
        webview.FOLDER_DIALOG,
        directory="",
        allow_multiple=False,
    )
    if not result or len(result) == 0:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": "No output directory selected."},
        )
        await ws.send_json(response)
        return

    output_dir = result[0]

    # Find GamePass container path
    try:
        container_path = find_container_path()
    except Exception as e:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": f"Could not find GamePass installation: {e}"},
        )
        await ws.send_json(response)
        return

    await ws_callback("Reading container index...")
    container_index = read_container_index(container_path)
    containers = container_index.get_save_containers(save_id)

    if not containers.get("Level"):
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": f"No Level container found for save: {save_id}"},
        )
        await ws.send_json(response)
        return

    save_dir = os.path.join(output_dir, save_id)
    os.makedirs(save_dir, exist_ok=True)
    players_dir = os.path.join(save_dir, "Players")
    os.makedirs(players_dir, exist_ok=True)

    for key, container in containers.items():
        container_dir = os.path.join(
            container_path,
            container.container_uuid.bytes_le.hex().upper(),
        )

        if not os.path.exists(container_dir):
            logger.warning("Container directory not found, skipping: %s", container_dir)
            continue

        file_data = None
        for filename in os.listdir(container_dir):
            if filename.startswith("container."):
                file_path = os.path.join(container_dir, filename)
                try:
                    with open(file_path, "rb") as f:
                        file_list = ContainerFileList.from_stream(f)
                        if file_list.files:
                            file_data = file_list.files[0].data
                except Exception as e:
                    logger.warning("Failed to read container %s: %s", file_path, e)

        if not file_data:
            continue

        if key == "Level":
            await ws_callback("Converting Level.sav to Steam format...")
            converted = recompress_to_steam(file_data)
            with open(os.path.join(save_dir, "Level.sav"), "wb") as f:
                f.write(converted)
        elif key == "LevelMeta":
            await ws_callback("Extracting LevelMeta.sav...")
            with open(os.path.join(save_dir, "LevelMeta.sav"), "wb") as f:
                f.write(recompress_to_steam(file_data))
        elif key == "LocalData":
            await ws_callback("Extracting LocalData.sav...")
            with open(os.path.join(save_dir, "LocalData.sav"), "wb") as f:
                f.write(recompress_to_steam(file_data))
        elif key == "WorldOption":
            await ws_callback("Extracting WorldOption.sav...")
            with open(os.path.join(save_dir, "WorldOption.sav"), "wb") as f:
                f.write(recompress_to_steam(file_data))
        elif key.startswith("Players-"):
            player_id = key.split("Players-")[1]
            await ws_callback(f"Extracting player {player_id}...")
            with open(os.path.join(players_dir, f"{player_id}.sav"), "wb") as f:
                f.write(recompress_to_steam(file_data))

    await ws_callback("Conversion complete!")
    response = build_response(
        MessageType.CONVERT_SAVE_FORMAT,
        {
            "message": "GamePass save extracted to Steam format",
            "output_path": save_dir,
        },
    )
    await ws.send_json(response)


async def _loaded_save_to_gamepass(app_state, ws: WebSocket, ws_callback):
    if app_state.save_type == SaveType.GAMEPASS:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": "Save is already in GamePass format."},
        )
        await ws.send_json(response)
        return

    await ws_callback("Finding GamePass container path...")
    try:
        container_path = find_container_path()
    except Exception as e:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": f"Could not find GamePass installation: {e}"},
        )
        await ws.send_json(response)
        return

    await ws_callback("Creating backup of GamePass containers...")
    backup_container_path(container_path)

    await ws_callback("Reading container index...")
    container_index = read_container_index(container_path)
    # Only remove EggTest ghost entries, not all orphaned containers
    container_index.containers = [
        c
        for c in container_index.containers
        if not c.container_name.startswith("EggTest")
    ]

    new_save_id = uuid.uuid4().hex.upper()

    await ws_callback("Converting Level.sav...")
    level_data = app_state.save_file.sav()

    await ws_callback("Converting player save files...")
    player_sav_data = app_state.save_file.player_gvas_files()

    # Create Level container
    await ws_callback("Creating GamePass containers...")
    level_container = create_new_container(container_path, new_save_id, level_data)
    container_index.containers.append(level_container)

    # Create LevelMeta container
    level_meta_data = app_state.save_file.level_meta_sav()
    if level_meta_data:
        meta_container = create_new_container(
            container_path, new_save_id, level_meta_data, container_suffix="LevelMeta"
        )
        container_index.containers.append(meta_container)

    # Create player containers (UUID without dashes to match game expectations)
    for player_uuid, player_files in player_sav_data.items():
        player_id = player_uuid.hex.upper()
        if "sav" in player_files:
            player_container = create_new_container(
                container_path,
                new_save_id,
                player_files["sav"],
                container_suffix=f"Players-{player_id}",
            )
            container_index.containers.append(player_container)

        if player_files.get("dps"):
            dps_container = create_new_container(
                container_path,
                new_save_id,
                player_files["dps"],
                container_suffix=f"Players-{player_id}_dps",
            )
            container_index.containers.append(dps_container)

    await ws_callback("Writing container index...")
    container_index.write_file(container_path)

    await ws_callback("Conversion complete!")
    response = build_response(
        MessageType.CONVERT_SAVE_FORMAT,
        {
            "message": f"Save converted to GamePass format (ID: {new_save_id})",
            "save_id": new_save_id,
        },
    )
    await ws.send_json(response)


async def _loaded_save_to_steam(app_state, ws: WebSocket, ws_callback):
    if app_state.save_type == SaveType.STEAM:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": "Save is already in Steam format."},
        )
        await ws.send_json(response)
        return

    # Ask user for output directory
    window = app_state.webview_window
    if not window:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": "Desktop mode required for Steam directory selection."},
        )
        await ws.send_json(response)
        return

    result = window.create_file_dialog(
        webview.FOLDER_DIALOG,
        directory="",
        allow_multiple=False,
    )

    if not result or len(result) == 0:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": "No output directory selected."},
        )
        await ws.send_json(response)
        return

    output_dir = result[0]
    save_id = uuid.uuid4().hex.upper()
    save_dir = os.path.join(output_dir, save_id)

    await _write_steam_save(app_state, save_dir, ws, ws_callback)


async def _write_steam_save(app_state, save_dir: str, ws: WebSocket, ws_callback):
    os.makedirs(save_dir, exist_ok=True)
    players_dir = os.path.join(save_dir, "Players")
    os.makedirs(players_dir, exist_ok=True)

    await ws_callback("Writing Level.sav...")
    level_path = os.path.join(save_dir, "Level.sav")
    level_data = app_state.save_file.sav()
    with open(level_path, "wb") as f:
        f.write(level_data)

    await ws_callback("Writing LevelMeta.sav...")
    level_meta_data = app_state.save_file.level_meta_sav()
    if level_meta_data:
        level_meta_path = os.path.join(save_dir, "LevelMeta.sav")
        with open(level_meta_path, "wb") as f:
            f.write(level_meta_data)

    await ws_callback("Writing player save files...")
    player_sav_data = app_state.save_file.player_gvas_files()
    for player_uuid, player_files in player_sav_data.items():
        if "sav" in player_files:
            player_path = os.path.join(players_dir, f"{player_uuid.hex.upper()}.sav")
            with open(player_path, "wb") as f:
                f.write(player_files["sav"])

    await ws_callback("Conversion complete!")
    response = build_response(
        MessageType.CONVERT_SAVE_FORMAT,
        {
            "message": f"Save converted to Steam format at: {save_dir}",
            "output_path": save_dir,
        },
    )
    await ws.send_json(response)


def _prompt_standalone_paths(app_state, target_format: str):
    """Prompt user for source and output directories via file dialogs."""
    window = app_state.webview_window
    if not window:
        return None, None

    # Source: Steam save dir or GamePass container dir
    if target_format == "gamepass":
        # Source is Steam, need to select a folder containing Level.sav
        source_result = window.create_file_dialog(
            webview.FOLDER_DIALOG,
            directory="",
            allow_multiple=False,
        )
    else:
        # Source is GamePass, need to select the container directory
        source_result = window.create_file_dialog(
            webview.OPEN_DIALOG,
            directory="",
            allow_multiple=False,
            file_types=("Container Index (*.index)", "All files (*.*)"),
        )

    if not source_result or len(source_result) == 0:
        return None, None

    source_path = source_result[0]
    # For GamePass source, we need the directory containing containers.index
    if target_format == "steam" and os.path.isfile(source_path):
        source_path = os.path.dirname(source_path)

    # Output: where to write the converted files
    if target_format == "steam":
        # Output is a folder for Steam save files
        output_result = window.create_file_dialog(
            webview.FOLDER_DIALOG,
            directory="",
            allow_multiple=False,
        )
    else:
        # Output is the GamePass container path
        try:
            output_path = find_container_path()
            return source_path, output_path
        except Exception:
            output_result = window.create_file_dialog(
                webview.OPEN_DIALOG,
                directory="",
                allow_multiple=False,
                file_types=("Container Index (*.index)", "All files (*.*)"),
            )

    if not output_result or len(output_result) == 0:
        return None, None

    output_path = output_result[0]
    if os.path.isfile(output_path):
        output_path = os.path.dirname(output_path)

    return source_path, output_path


async def _convert_standalone(
    source_path: str,
    output_path: str,
    target_format: str,
    ws: WebSocket,
    ws_callback,
):
    """Convert save files between formats without loading into the editor."""
    if target_format == "steam":
        await _standalone_gamepass_to_steam(source_path, output_path, ws, ws_callback)
    elif target_format == "gamepass":
        await _standalone_steam_to_gamepass(source_path, output_path, ws, ws_callback)
    else:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": f"Unknown target format: {target_format}"},
        )
        await ws.send_json(response)


async def _standalone_gamepass_to_steam(
    source_path: str, output_path: str, ws: WebSocket, ws_callback
):
    """Extract GamePass containers to Steam directory format."""
    await ws_callback("Reading GamePass container index...")
    container_index = read_container_index(source_path)

    # Get all unique save IDs
    save_ids = set()
    for container in container_index.containers:
        parts = container.container_name.split("-", 1)
        if len(parts) >= 2:
            save_ids.add(parts[0])

    if not save_ids:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": "No saves found in GamePass containers."},
        )
        await ws.send_json(response)
        return

    for save_id in save_ids:
        containers = container_index.get_save_containers(save_id)
        if not containers.get("Level"):
            continue

        save_dir = os.path.join(output_path, save_id)
        os.makedirs(save_dir, exist_ok=True)
        players_dir = os.path.join(save_dir, "Players")
        os.makedirs(players_dir, exist_ok=True)

        for key, container in containers.items():
            container_dir = os.path.join(
                source_path,
                container.container_uuid.bytes_le.hex().upper(),
            )

            if not os.path.exists(container_dir):
                logger.warning(
                    "Container directory not found, skipping: %s", container_dir
                )
                continue

            # Read container files
            file_data = None
            for filename in os.listdir(container_dir):
                if filename.startswith("container."):
                    file_path = os.path.join(container_dir, filename)
                    try:
                        with open(file_path, "rb") as f:
                            file_list = ContainerFileList.from_stream(f)
                            if file_list.files:
                                file_data = file_list.files[0].data
                    except Exception as e:
                        logger.warning(
                            "Failed to read container file %s: %s",
                            file_path,
                            e,
                        )

            if not file_data:
                continue

            if key == "Level":
                await ws_callback(f"Converting Level.sav for {save_id}...")
                with open(os.path.join(save_dir, "Level.sav"), "wb") as f:
                    f.write(recompress_to_steam(file_data))
            elif key == "LevelMeta":
                await ws_callback(f"Extracting LevelMeta.sav for {save_id}...")
                with open(os.path.join(save_dir, "LevelMeta.sav"), "wb") as f:
                    f.write(recompress_to_steam(file_data))
            elif key == "LocalData":
                await ws_callback(f"Extracting LocalData.sav for {save_id}...")
                with open(os.path.join(save_dir, "LocalData.sav"), "wb") as f:
                    f.write(recompress_to_steam(file_data))
            elif key == "WorldOption":
                await ws_callback(f"Extracting WorldOption.sav for {save_id}...")
                with open(os.path.join(save_dir, "WorldOption.sav"), "wb") as f:
                    f.write(recompress_to_steam(file_data))
            elif key.startswith("Players-"):
                player_id = key.split("Players-")[1]
                await ws_callback(f"Extracting player {player_id}...")
                with open(os.path.join(players_dir, f"{player_id}.sav"), "wb") as f:
                    f.write(recompress_to_steam(file_data))

    await ws_callback("Conversion complete!")
    response = build_response(
        MessageType.CONVERT_SAVE_FORMAT,
        {
            "message": f"GamePass saves extracted to Steam format at: {output_path}",
            "output_path": output_path,
        },
    )
    await ws.send_json(response)


async def _standalone_steam_to_gamepass(
    source_path: str, output_path: str, ws: WebSocket, ws_callback
):
    """Package Steam save directory into GamePass container format."""
    # Validate source is a Steam save directory
    level_sav = os.path.join(source_path, "Level.sav")
    if not os.path.exists(level_sav):
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {
                "error": "Level.sav not found in source directory. Is this a valid Steam save?"
            },
        )
        await ws.send_json(response)
        return

    await ws_callback("Reading GamePass container index...")
    try:
        container_index = read_container_index(output_path)
    except Exception as e:
        response = build_response(
            MessageType.CONVERT_SAVE_FORMAT,
            {"error": f"Could not read GamePass container index: {e}"},
        )
        await ws.send_json(response)
        return

    await ws_callback("Creating backup...")
    backup_container_path(output_path)
    # Only remove EggTest ghost entries, not all orphaned containers
    container_index.containers = [
        c
        for c in container_index.containers
        if not c.container_name.startswith("EggTest")
    ]

    new_save_id = uuid.uuid4().hex.upper()

    # Read and create Level container
    await ws_callback("Creating Level container...")
    with open(level_sav, "rb") as f:
        level_data = f.read()
    level_container = create_new_container(output_path, new_save_id, level_data)
    container_index.containers.append(level_container)

    # Read and create LevelMeta container
    level_meta_path = os.path.join(source_path, "LevelMeta.sav")
    if os.path.exists(level_meta_path):
        await ws_callback("Creating LevelMeta container...")
        with open(level_meta_path, "rb") as f:
            meta_data = f.read()
        meta_container = create_new_container(
            output_path, new_save_id, meta_data, container_suffix="LevelMeta"
        )
        container_index.containers.append(meta_container)

    # Read and create Player containers
    players_dir = os.path.join(source_path, "Players")
    if os.path.exists(players_dir):
        for player_file in Path(players_dir).glob("*.sav"):
            player_id = player_file.stem
            await ws_callback(f"Creating player container: {player_id}...")
            with open(player_file, "rb") as f:
                player_data = f.read()
            player_container = create_new_container(
                output_path,
                new_save_id,
                player_data,
                container_suffix=f"Players-{player_id}",
            )
            container_index.containers.append(player_container)

    await ws_callback("Writing container index...")
    container_index.write_file(output_path)

    await ws_callback("Conversion complete!")
    response = build_response(
        MessageType.CONVERT_SAVE_FORMAT,
        {
            "message": f"Steam save imported to GamePass format (ID: {new_save_id})",
            "save_id": new_save_id,
        },
    )
    await ws.send_json(response)


# --- GamePass management handlers ---


async def delete_gamepass_save_handler(
    message: DeleteGamepassSaveMessage, ws: WebSocket
):
    """Delete all containers for a GamePass save."""
    save_id = message.data.save_id

    try:
        container_path = find_container_path()
    except Exception as e:
        response = build_response(
            MessageType.DELETE_GAMEPASS_SAVE,
            {"error": f"Could not find GamePass installation: {e}"},
        )
        await ws.send_json(response)
        return

    backup_container_path(container_path)
    container_index = read_container_index(container_path)

    # Find and remove all containers for this save
    to_remove = [
        c
        for c in container_index.containers
        if c.container_name.startswith(f"{save_id}-")
    ]

    if not to_remove:
        response = build_response(
            MessageType.DELETE_GAMEPASS_SAVE,
            {"error": f"No containers found for save: {save_id}"},
        )
        await ws.send_json(response)
        return

    for container in to_remove:
        container_index.containers.remove(container)
        container_dir = os.path.join(
            container_path, container.container_uuid.bytes_le.hex().upper()
        )
        if os.path.exists(container_dir):
            shutil.rmtree(container_dir)
            logger.info("Deleted container directory: %s", container_dir)

    container_index.write_file(container_path)
    logger.info("Deleted %d containers for save %s", len(to_remove), save_id)

    response = build_response(
        MessageType.DELETE_GAMEPASS_SAVE,
        {"message": f"Deleted save with {len(to_remove)} containers"},
    )
    await ws.send_json(response)


async def delete_gamepass_player_handler(
    message: DeleteGamepassPlayerMessage, ws: WebSocket
):
    """Delete player containers from a GamePass save."""
    save_id = message.data.save_id
    player_id = message.data.player_id

    try:
        container_path = find_container_path()
    except Exception as e:
        response = build_response(
            MessageType.DELETE_GAMEPASS_PLAYER,
            {"error": f"Could not find GamePass installation: {e}"},
        )
        await ws.send_json(response)
        return

    backup_container_path(container_path)
    container_index = read_container_index(container_path)

    # Find player containers (main + dps)
    player_suffix = f"Players-{player_id}"
    to_remove = [
        c
        for c in container_index.containers
        if c.container_name.startswith(f"{save_id}-")
        and player_suffix in c.container_name
    ]

    if not to_remove:
        response = build_response(
            MessageType.DELETE_GAMEPASS_PLAYER,
            {"error": f"No containers found for player: {player_id}"},
        )
        await ws.send_json(response)
        return

    for container in to_remove:
        container_index.containers.remove(container)
        container_dir = os.path.join(
            container_path, container.container_uuid.bytes_le.hex().upper()
        )
        if os.path.exists(container_dir):
            shutil.rmtree(container_dir)
            logger.info("Deleted player container: %s", container_dir)

    container_index.write_file(container_path)
    logger.info(
        "Deleted %d containers for player %s in save %s",
        len(to_remove),
        player_id,
        save_id,
    )

    response = build_response(
        MessageType.DELETE_GAMEPASS_PLAYER,
        {"message": f"Deleted player {player_id}"},
    )
    await ws.send_json(response)


async def rename_gamepass_world_handler(
    message: RenameGamepassWorldMessage, ws: WebSocket
):
    """Rename a GamePass save world by modifying LevelMeta."""
    save_id = message.data.save_id
    new_name = message.data.new_name

    try:
        container_path = find_container_path()
    except Exception as e:
        response = build_response(
            MessageType.RENAME_GAMEPASS_WORLD,
            {"error": f"Could not find GamePass installation: {e}"},
        )
        await ws.send_json(response)
        return

    backup_container_path(container_path)
    container_index = read_container_index(container_path)
    containers = container_index.get_save_containers(save_id)
    level_meta_container = containers.get("LevelMeta")

    if not level_meta_container:
        response = build_response(
            MessageType.RENAME_GAMEPASS_WORLD,
            {"error": f"No LevelMeta found for save: {save_id}"},
        )
        await ws.send_json(response)
        return

    # Read the LevelMeta container data
    meta_dir = os.path.join(
        container_path, level_meta_container.container_uuid.bytes_le.hex().upper()
    )
    meta_data = None
    for filename in os.listdir(meta_dir):
        if filename.startswith("container."):
            with open(os.path.join(meta_dir, filename), "rb") as f:
                file_list = ContainerFileList.from_stream(f)
                if file_list.files:
                    meta_data = file_list.files[0].data
            break

    if not meta_data:
        response = build_response(
            MessageType.RENAME_GAMEPASS_WORLD,
            {"error": "Could not read LevelMeta data"},
        )
        await ws.send_json(response)
        return

    # Modify the world name
    level_meta = SaveManager().load_level_meta(meta_data)
    level_meta.properties["SaveData"]["value"]["WorldName"]["value"] = new_name
    modified_data = SaveManager().sav(level_meta)

    # Create new container with modified data
    new_container = create_new_container(
        container_path, save_id, modified_data, container_suffix="LevelMeta"
    )
    container_index.containers.append(new_container)
    container_index.write_file(container_path)

    logger.info("Renamed world for save %s to: %s", save_id, new_name)

    response = build_response(
        MessageType.RENAME_GAMEPASS_WORLD,
        {"message": f"World renamed to '{new_name}'"},
    )
    await ws.send_json(response)
