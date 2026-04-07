import os

import webview
from fastapi import WebSocket

from palworld_save_pal.game.player_transfer import transfer_player
from palworld_save_pal.game.save_manager import SaveManager
from palworld_save_pal.state import get_app_state
from palworld_save_pal.utils.file_manager import FileManager
from palworld_save_pal.ws.messages import (
    GetSourcePlayersMessage,
    LoadSourceSaveMessage,
    MessageType,
    TransferPlayerMessage,
    UnloadSourceSaveMessage,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def _load_steam_save(save_path: str, label: str, ws_callback) -> tuple[SaveManager, dict]:
    """Load a Steam save and return (SaveManager, validation_info).

    validation_info contains level_sav, level_meta, and players_dir paths
    needed for writing the save back to disk after modifications.
    """
    if os.path.isdir(save_path):
        save_path = os.path.join(save_path, "Level.sav")

    validation = FileManager.validate_steam_save_directory(save_path)
    if not validation.valid:
        raise ValueError(validation.error)

    await ws_callback(f"Loading {label} Level.sav...")

    with open(validation.level_sav, "rb") as f:
        level_sav = f.read()

    level_meta = None
    if validation.level_meta:
        with open(validation.level_meta, "rb") as f:
            level_meta = f.read()

    player_file_refs = FileManager.get_player_save_paths(validation.players_dir)

    save_manager = await SaveManager(level_sav_path=validation.level_sav).load_sav_files(
        level_sav, player_file_refs, level_meta, ws_callback
    )

    save_info = {
        "level_sav": validation.level_sav,
        "level_meta": validation.level_meta,
        "players_dir": validation.players_dir,
        "save_dir": os.path.dirname(validation.level_sav),
    }

    return save_manager, save_info


def _prompt_folder(app_state):
    window = app_state.webview_window
    if not window:
        raise ValueError("Desktop mode required for file selection.")
    result = window.create_file_dialog(
        webview.FOLDER_DIALOG,
        directory="",
        allow_multiple=False,
    )
    if not result or len(result) == 0:
        raise ValueError("No directory selected.")
    return result[0]


async def load_source_save_handler(message: LoadSourceSaveMessage, ws: WebSocket):
    app_state = get_app_state()
    role = message.data.role
    is_target = role == "target"

    async def ws_callback(msg: str):
        response = build_response(MessageType.PROGRESS_MESSAGE, msg)
        await ws.send_json(response)

    try:
        if message.data.type != "steam":
            response = build_response(
                MessageType.LOAD_SOURCE_SAVE,
                {"error": "Only Steam saves are supported."},
            )
            await ws.send_json(response)
            return

        save_path = message.data.path
        if save_path == "__select__":
            save_path = _prompt_folder(app_state)

        label = "target" if is_target else "source"
        save_manager, save_info = await _load_steam_save(save_path, label, ws_callback)
        summaries = save_manager.get_player_summaries()

        if is_target:
            app_state.target_transfer_save = save_manager
            app_state.target_transfer_summaries = summaries
            app_state.target_transfer_save_info = save_info
        else:
            app_state.source_save_file = save_manager
            app_state.source_player_summaries = summaries

        response = build_response(
            MessageType.LOAD_SOURCE_SAVE,
            {
                "success": True,
                "role": role,
                "player_count": len(summaries),
                "world_name": save_manager.world_name,
            },
        )
        await ws.send_json(response)

    except Exception as error:
        logger.error("Failed to load %s save: %s", role, error, exc_info=True)
        response = build_response(
            MessageType.LOAD_SOURCE_SAVE,
            {"error": str(error)},
        )
        await ws.send_json(response)


async def get_source_players_handler(message: GetSourcePlayersMessage, ws: WebSocket):
    app_state = get_app_state()
    response = build_response(
        MessageType.GET_SOURCE_PLAYERS,
        {
            "source": app_state.source_player_summaries
            if app_state.source_save_file
            else {},
            "target": app_state.target_transfer_summaries
            if app_state.target_transfer_save
            else {},
        },
    )
    await ws.send_json(response)


async def transfer_player_handler(message: TransferPlayerMessage, ws: WebSocket):
    app_state = get_app_state()

    target_save = app_state.target_transfer_save or app_state.save_file
    if not target_save:
        response = build_response(
            MessageType.TRANSFER_PLAYER, {"error": "No target save loaded."}
        )
        await ws.send_json(response)
        return

    if not app_state.source_save_file:
        response = build_response(
            MessageType.TRANSFER_PLAYER, {"error": "No source save loaded."}
        )
        await ws.send_json(response)
        return

    async def ws_callback(msg: str):
        progress = build_response(MessageType.PROGRESS_MESSAGE, msg)
        await ws.send_json(progress)

    data = message.data
    result = await transfer_player(
        source=app_state.source_save_file,
        target=target_save,
        source_player_uid=data.source_player_uid,
        target_player_uid=data.target_player_uid,
        transfer_character=data.transfer_character,
        transfer_inventory=data.transfer_inventory,
        transfer_pals=data.transfer_pals,
        transfer_tech=data.transfer_tech,
        transfer_appearance=data.transfer_appearance,
        ws_callback=ws_callback,
    )

    if result.get("success"):
        if app_state.target_transfer_save:
            app_state.target_transfer_summaries = target_save.get_player_summaries()

            # Auto-save standalone target to disk since there's no separate save button
            save_info = app_state.target_transfer_save_info
            if save_info:
                await ws_callback("Saving modified target save to disk...")
                import shutil
                import time

                # Create backup
                backup_dir = os.path.join(save_info["save_dir"], "backups", "transfer")
                os.makedirs(backup_dir, exist_ok=True)
                timestamp = time.strftime("%Y-%m-%d-%H-%M-%S")
                backup_path = os.path.join(backup_dir, f"backup_{timestamp}")
                if not os.path.exists(backup_path):
                    shutil.copytree(save_info["save_dir"], backup_path,
                                    ignore=shutil.ignore_patterns("backups"))

                target_save.to_level_sav_file(save_info["level_sav"])
                if save_info.get("level_meta"):
                    target_save.to_level_meta_sav_file(save_info["level_meta"])
                target_save.to_player_sav_files(save_info["players_dir"])

                await ws_callback("Target save written to disk.")
                result["saved_to"] = save_info["save_dir"]
        else:
            app_state.player_summaries = target_save.get_player_summaries()
            app_state.guild_summaries = target_save.get_guild_summaries()

    response = build_response(MessageType.TRANSFER_PLAYER, result)
    await ws.send_json(response)


async def unload_source_save_handler(message: UnloadSourceSaveMessage, ws: WebSocket):
    app_state = get_app_state()
    app_state.source_save_file = None
    app_state.source_player_summaries = {}
    app_state.target_transfer_save = None
    app_state.target_transfer_summaries = {}
    app_state.target_transfer_save_info = None
    response = build_response(MessageType.UNLOAD_SOURCE_SAVE, {"success": True})
    await ws.send_json(response)
