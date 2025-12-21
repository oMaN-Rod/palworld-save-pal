import base64
import io
import os
from typing import Dict
import uuid
import zipfile
from fastapi import WebSocket
from palworld_save_pal.ws.messages import (
    BaseMessage,
    DownloadSaveFileMessage,
    MessageType,
    UpdateSaveFileMessage,
    LoadZipFileMessage,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.logging_config import create_logger
import datetime

logger = create_logger(__name__)


async def update_save_file_handler(message: UpdateSaveFileMessage, ws: WebSocket):
    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    modified_pals = message.data.modified_pals if message.data.modified_pals else None
    modified_players = (
        message.data.modified_players if message.data.modified_players else None
    )
    modified_guilds = (
        message.data.modified_guilds if message.data.modified_guilds else None
    )
    modified_dps_pals = (
        message.data.modified_dps_pals if message.data.modified_dps_pals else None
    )
    modified_gps_pals = (
        message.data.modified_gps_pals if message.data.modified_gps_pals else None
    )
    app_state = get_app_state()
    save_file = app_state.save_file

    if not save_file:
        raise ValueError("No save file loaded")

    if modified_pals:
        await save_file.update_pals(modified_pals, ws_callback)
    if modified_players:
        await save_file.update_players(modified_players, ws_callback)
    if modified_guilds:
        await save_file.update_guilds(modified_guilds, ws_callback)
    if modified_dps_pals:
        await save_file.update_dps_pals(modified_dps_pals, ws_callback)
    if modified_gps_pals:
        await save_file.update_gps_pals(modified_gps_pals, ws_callback)

    app_state.players = save_file.get_players()
    response = build_response(MessageType.UPDATE_SAVE_FILE, "Changes saved")
    await ws.send_json(response)


async def download_save_file_handler(_: DownloadSaveFileMessage, ws: WebSocket):
    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    app_state = get_app_state()
    save_file = app_state.save_file

    if not save_file:
        raise ValueError("No save file loaded")

    await ws_callback("Generating save files in memory... 💾")

    level_sav_data = save_file.sav()
    logger.debug("Got Level.sav data (%d bytes)", len(level_sav_data))

    player_sav_files = save_file.player_gvas_files()
    logger.debug("Got data for %d players", len(player_sav_files))

    await ws_callback("Creating ZIP archive... 🤏")
    zip_buffer = io.BytesIO()
    with zipfile.ZipFile(zip_buffer, "w", zipfile.ZIP_DEFLATED) as zipf:
        zipf.writestr("Level.sav", level_sav_data)
        logger.debug("Added Level.sav to ZIP")

        player_count = 0
        for player_id, files_data in player_sav_files.items():
            player_uuid_str = str(player_id).replace("-", "")

            if files_data.get("sav"):
                player_sav_path = f"Players/{player_uuid_str}.sav"
                zipf.writestr(player_sav_path, files_data["sav"])
                logger.debug("Added %s to ZIP", player_sav_path)
                player_count += 1
            else:
                logger.warning("Missing main save data for player %s", player_id)

            if files_data.get("dps") is not None:
                player_dps_path = f"Players/{player_uuid_str}_dps.sav"
                zipf.writestr(player_dps_path, files_data["dps"])
                logger.debug("Added %s to ZIP", player_dps_path)

    await ws_callback(
        f"Archive created with Level.sav and {player_count} player(s) data. Encoding..."
    )

    zip_data = zip_buffer.getvalue()
    zip_buffer.close()
    logger.debug("ZIP archive size: %d bytes", len(zip_data))

    encoded_zip_data = base64.b64encode(zip_data).decode("utf-8")
    logger.debug("Encoded ZIP data length: %d", len(encoded_zip_data))

    now = datetime.datetime.now()
    timestamp_str = now.strftime("%Y%m%d_%H%M%S")
    response_data = [
        {
            "name": f"{app_state.save_file.world_name or 'PSP'}_{timestamp_str}.zip",
            "content": encoded_zip_data,
        }
    ]

    await ws_callback("Sending ZIP file to client... 🚀")
    response = build_response(MessageType.DOWNLOAD_SAVE_FILE, response_data)
    await ws.send_json(response)
    logger.info("Sent ZIP archive to client")


async def load_zip_file_handler(message: LoadZipFileMessage, ws: WebSocket):
    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    app_state = get_app_state()
    zip_data = bytes(message.data)

    with zipfile.ZipFile(io.BytesIO(zip_data), "r") as zip_ref:
        file_list = zip_ref.namelist()
        if file_list is None:
            raise ValueError("Zip file is empty")

        save_id = file_list[0].split("/")[0]
        nested = not any(f == "Level.sav" for f in file_list)
        level_sav = f"{save_id}/Level.sav" if nested else "Level.sav"
        level_meta_sav = f"{save_id}/LevelMeta.sav" if nested else "LevelMeta.sav"
        players_folder = f"{save_id}/Players/" if nested else "Players/"

        if level_sav not in file_list:
            raise ValueError("Zip file does not contain 'Level.sav'")

        if not any(f.startswith(players_folder) for f in file_list):
            raise ValueError("Zip file does not contain 'Players' folder")

        level_sav_data = zip_ref.read(level_sav)

        level_meta_data = None
        if level_meta_sav in file_list:
            level_meta_data = zip_ref.read(level_meta_sav)

        player_saves: Dict[uuid.UUID, Dict[str, bytes]] = {}
        for f in (
            file for file in file_list if "Players" in file and file.endswith(".sav")
        ):
            dps = False
            player_id = os.path.splitext(os.path.basename(f))[0]
            if "_dps" in f:
                player_id = player_id.replace("_dps", "")
                dps = True
            try:
                player_uuid = uuid.UUID(player_id)
            except ValueError:
                logger.warning("Skipping invalid player file name: %s", f)
                continue

            if player_uuid not in player_saves:
                player_saves[player_uuid] = {}
            save_type = "dps" if dps else "sav"
            player_saves[player_uuid][save_type] = zip_ref.read(f)

        if not player_saves:
            raise ValueError("No valid player save files found in the 'Players' folder")

        await app_state.process_save_files(
            sav_id=save_id,
            level_sav=level_sav_data,
            level_meta=level_meta_data,
            player_savs=player_saves,
            ws_callback=ws_callback,
        )

    data = {
        "level": app_state.save_file.world_name,
        "players": [str(p) for p in app_state.players.keys()],
        "name": app_state.save_file.level_sav_path,
        "size": app_state.save_file.size,
        "type": app_state.save_type.name.lower(),
        "local": app_state.local,
    }

    await ws_callback(
        "Zip file uploaded and processed successfully, results coming right up!"
    )

    response = build_response(MessageType.LOADED_SAVE_FILES, data)
    await ws.send_json(response)

    response = build_response(MessageType.GET_PLAYERS, app_state.players)
    await ws.send_json(response)

    response = build_response(MessageType.GET_GUILDS, app_state.guilds)
    await ws.send_json(response)


async def reload_mounted_save_handler(message: BaseMessage, ws: WebSocket):
    """Reload save files from mounted directory without restarting container."""
    import os
    from palworld_save_pal.utils.auto_loader import check_mounted_saves
    
    async def ws_callback(message: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, message)
        await ws.send_json(response)

    app_state = get_app_state()
    mount_path = os.getenv("SAVE_MOUNT_PATH", "/app/saves")
    
    await ws_callback(f"Checking for saves in {mount_path}...")
    
    save_data = check_mounted_saves(mount_path)
    if not save_data:
        error_msg = f"No valid save files found in {mount_path}"
        logger.error(error_msg)
        response = build_response(MessageType.ERROR, error_msg)
        await ws.send_json(response)
        return
    
    await ws_callback("Reloading save files...")
    
    try:
        await app_state.process_save_files(
            sav_id=save_data["save_id"],
            level_sav=save_data["level_sav"],
            level_meta=save_data["level_meta"],
            player_savs=save_data["player_saves"],
            ws_callback=ws_callback,
            local=True,
        )
        
        data = {
            "level": app_state.save_file.world_name,
            "players": [str(p) for p in app_state.players.keys()],
            "name": app_state.save_file.level_sav_path,
            "size": app_state.save_file.size,
            "type": app_state.save_type.name.lower(),
            "local": app_state.local,
        }
        
        await ws_callback("✅ Save reloaded successfully!")
        response = build_response(MessageType.LOADED_SAVE_FILES, data)
        await ws.send_json(response)
        
        response = build_response(MessageType.GET_PLAYERS, app_state.players)
        await ws.send_json(response)

        response = build_response(MessageType.GET_GUILDS, app_state.guilds)
        await ws.send_json(response)
        
        logger.info("Save files reloaded successfully from mounted directory")
        
    except Exception as e:
        error_msg = f"Failed to reload save files: {str(e)}"
        logger.error(error_msg, exc_info=True)
        response = build_response(MessageType.ERROR, error_msg)
        await ws.send_json(response)
