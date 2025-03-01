import base64
import io
import os
import uuid
import zipfile
from fastapi import WebSocket
from palworld_save_pal.ws.messages import (
    DownloadSaveFileMessage,
    MessageType,
    UpdateSaveFileMessage,
    LoadZipFileMessage,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.logging_config import create_logger

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
    await ws_callback("Compressing GVAS to sav 💪...")
    sav_file = save_file.sav()
    await ws_callback("Encoding sav file to base64 🤖, get ready here it comes...")
    encoded_data = base64.b64encode(sav_file).decode("utf-8")
    data = [{
        "name": "Level.sav",
        "content": encoded_data,
    }]

    # Prep player save files
    player_savs = save_file.player_savs()
    for player_id, save_file in player_savs.items():
        await ws_callback(f"Sending over {player_id}'s sav! 💪...")
        encoded_data = base64.b64encode(sav_file).decode("utf-8")
        player_data = {
            "name": f"{player_id}.sav",
            "content": encoded_data,
        }
        data.append(player_data)
    
    response = build_response(MessageType.DOWNLOAD_SAVE_FILE, data)
    await ws.send_json(response)


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

        player_files = [
            f for f in file_list if f.startswith(players_folder) and f.endswith(".sav")
        ]

        if not player_files:
            raise ValueError(f"No player save files found in the 'Players' folder")

        player_data = {}
        for player_file in player_files:
            player_id = os.path.splitext(os.path.basename(player_file))[0]
            player_uuid = uuid.UUID(player_id)
            player_data[player_uuid] = zip_ref.read(player_file)

        await app_state.process_save_files(
            sav_id=save_id,
            level_sav=level_sav_data,
            level_meta=level_meta_data,
            player_savs=player_data,
            ws_callback=ws_callback,
        )

    data = {
        "level": app_state.save_file.world_name,
        "players": [str(p) for p in app_state.players.keys()],
        "name": app_state.save_file.name,
        "size": app_state.save_file.size,
        "type": app_state.save_type.name.lower(),
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
