import glob
import os
import traceback
from pathlib import Path

from fastapi import WebSocket

from palworld_save_pal.db.ctx.servers import ServerDBService
from palworld_save_pal.services.docker_service import DockerService
from palworld_save_pal.services.native_server_service import NativeServerService
from palworld_save_pal.utils.file_manager import FileManager
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.ws.messages import (
    CreateServerMessage,
    DeleteServerMessage,
    GetServerMessage,
    GetServerStatsMessage,
    InstallServerModMessage,
    ListServerModsMessage,
    ListServersMessage,
    LoadServerSaveMessage,
    MessageType,
    ServerApiCallMessage,
    StartServerMessage,
    StopServerMessage,
    ToggleServerModMessage,
    UpdateServerMessage,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)


def _server_to_dict(server) -> dict:
    return {
        "id": server.id,
        "name": server.name,
        "container_name": server.container_name,
        "image_name": server.image_name,
        "server_type": server.server_type,
        "game_port": server.game_port,
        "query_port": server.query_port,
        "rest_api_port": server.rest_api_port,
        "data_volume_name": server.data_volume_name,
        "saves_path": server.saves_path,
        "mods_path": server.mods_path,
        "logicmods_path": server.logicmods_path,
        "nativemods_path": server.nativemods_path,
        "install_path": server.install_path,
        "steamcmd_path": server.steamcmd_path,
        "pid": server.pid,
        "launch_args": server.launch_args,
        "server_name": server.server_name,
        "server_description": server.server_description,
        "server_password": server.server_password,
        "admin_password": server.admin_password,
        "max_players": server.max_players,
        "env_vars": server.env_vars or {},
        "created_at": server.created_at.isoformat() if server.created_at else None,
        "updated_at": server.updated_at.isoformat() if server.updated_at else None,
    }


def _count_total_players(saves_path: str) -> int:
    """Count total players by scanning .sav files in the Players directory."""
    save_games = os.path.join(saves_path, "SaveGames", "0")
    if not os.path.isdir(save_games):
        return 0
    for world_dir in os.listdir(save_games):
        players_dir = os.path.join(save_games, world_dir, "Players")
        if os.path.isdir(players_dir):
            return len([f for f in os.listdir(players_dir) if f.endswith(".sav") and "_dps" not in f])
    return 0


def _get_server_status(server) -> dict:
    """Get server status based on server_type."""
    if server.server_type == "native":
        return NativeServerService.get_process_status(server.pid)
    return DockerService.get_container_status(server.container_name)


async def list_servers_handler(message: ListServersMessage, ws: WebSocket):
    try:
        servers = ServerDBService.list_servers()
        server_list = []
        for s in servers:
            data = _server_to_dict(s)
            status = _get_server_status(s)
            data["status"] = status
            data["total_players"] = _count_total_players(s.saves_path)
            # Try to get online player count for running servers
            if status and status.get("running"):
                try:
                    count = await DockerService.get_player_count(
                        "127.0.0.1", s.rest_api_port, s.admin_password
                    )
                    data["player_count"] = count
                except Exception as player_err:
                    logger.debug("Failed to get player count for %s: %s", s.name, player_err)
                    data["player_count"] = 0
            else:
                data["player_count"] = 0
            server_list.append(data)

        response = build_response(MessageType.LIST_SERVERS, {"servers": server_list})
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error listing servers: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to list servers: {e}"})
        await ws.send_json(response)


async def get_server_handler(message: GetServerMessage, ws: WebSocket):
    try:
        server = ServerDBService.get_server(message.data.server_id)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        data = _server_to_dict(server)
        status = _get_server_status(server)
        data["status"] = status
        data["total_players"] = _count_total_players(server.saves_path)

        if status and status.get("running"):
            try:
                count = await DockerService.get_player_count(
                    "127.0.0.1", server.rest_api_port, server.admin_password
                )
                data["player_count"] = count
            except Exception:
                data["player_count"] = 0
        else:
            data["player_count"] = 0

        response = build_response(MessageType.GET_SERVER, data)
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error getting server: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to get server: {e}"})
        await ws.send_json(response)


async def create_server_handler(message: CreateServerMessage, ws: WebSocket):
    try:
        data = message.data
        is_native = data.server_type == "native"

        # Validate port uniqueness
        allocated = ServerDBService.get_allocated_ports()
        for port in [data.game_port, data.query_port, data.rest_api_port]:
            if port in allocated:
                response = build_response(
                    MessageType.ERROR,
                    {"message": f"Port {port} is already allocated to another server"},
                )
                await ws.send_json(response)
                return

        if is_native:
            install_path = data.install_path
            if not install_path:
                response = build_response(
                    MessageType.ERROR,
                    {"message": "Install path is required for native servers"},
                )
                await ws.send_json(response)
                return

            async def send_progress(msg: str):
                await ws.send_json(build_response(
                    MessageType.SERVER_CREATION_PROGRESS,
                    {"message": msg},
                ))

            await send_progress("Validating server configuration...")

            server_data = {
                "name": data.name,
                "container_name": data.container_name,
                "image_name": "",
                "server_type": "native",
                "game_port": data.game_port,
                "query_port": data.query_port,
                "rest_api_port": data.rest_api_port,
                "data_volume_name": "",
                "install_path": install_path,
                "steamcmd_path": data.steamcmd_path,
                "launch_args": data.launch_args,
                "saves_path": NativeServerService.get_saves_path(install_path),
                "mods_path": NativeServerService.get_mods_path(install_path),
                "logicmods_path": NativeServerService.get_logicmods_path(install_path),
                "nativemods_path": NativeServerService.get_nativemods_path(install_path),
                "server_name": data.server_name,
                "server_description": data.server_description,
                "server_password": data.server_password,
                "admin_password": data.admin_password,
                "max_players": data.max_players,
                "env_vars": data.env_vars,
            }

            server = ServerDBService.create_server(server_data)

            # Find existing PalServer to copy from
            await send_progress("Searching for existing PalServer installation...")
            source_path = NativeServerService.find_existing_server(data.steamcmd_path, install_path)

            if source_path:
                await send_progress(f"Found existing server at {source_path}, copying files...")
            elif data.steamcmd_path:
                # Ensure SteamCMD is available
                await send_progress("Setting up SteamCMD...")
                steamcmd_dir = os.path.dirname(data.steamcmd_path) if data.steamcmd_path.endswith(".exe") else data.steamcmd_path
                await NativeServerService.ensure_steamcmd(steamcmd_dir)
                await send_progress("Downloading Palworld Dedicated Server via SteamCMD (this may take a while)...")

            success = NativeServerService.create_server(server, source_path)
            if not success:
                ServerDBService.delete_server(server.id)
                await send_progress("")
                response = build_response(
                    MessageType.ERROR,
                    {"message": "Failed to create native server installation"},
                )
                await ws.send_json(response)
                return

            await send_progress("Writing server configuration files...")
            # Config is already written by create_server, just signaling progress

            await send_progress("")  # Clear progress
            result = _server_to_dict(server)
            result["status"] = NativeServerService.get_process_status(server.pid)
            result["player_count"] = 0

        else:
            # Docker path (existing logic)
            base_path = os.path.join(os.getcwd(), "servers", data.container_name)

            server_data = {
                "name": data.name,
                "container_name": data.container_name,
                "image_name": data.image_name,
                "server_type": "docker",
                "game_port": data.game_port,
                "query_port": data.query_port,
                "rest_api_port": data.rest_api_port,
                "data_volume_name": f"psp-{data.container_name}-data",
                "saves_path": os.path.join(base_path, "saves"),
                "mods_path": os.path.join(base_path, "mods"),
                "logicmods_path": os.path.join(base_path, "logicmods"),
                "nativemods_path": os.path.join(base_path, "nativemods"),
                "server_name": data.server_name,
                "server_description": data.server_description,
                "server_password": data.server_password,
                "admin_password": data.admin_password,
                "max_players": data.max_players,
                "env_vars": data.env_vars,
            }

            server = ServerDBService.create_server(server_data)

            # Create and start Docker container
            DockerService.create_server(server)

            result = _server_to_dict(server)
            result["status"] = DockerService.get_container_status(server.container_name)
            result["player_count"] = 0

        response = build_response(MessageType.CREATE_SERVER, result)
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error creating server: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to create server: {e}"})
        await ws.send_json(response)


async def update_server_handler(message: UpdateServerMessage, ws: WebSocket):
    try:
        data = message.data

        # Get the old server state before updating
        old_server = ServerDBService.get_server(data.server_id)
        if not old_server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        server = ServerDBService.update_server(data.server_id, data.updates)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Failed to update server"})
            await ws.send_json(response)
            return

        # Check if env vars or ports changed — requires restart/recreation
        env_changed = data.updates.get("env_vars") is not None
        ports_changed = any(
            k in data.updates for k in ("game_port", "query_port", "rest_api_port")
        )
        identity_changed = any(
            k in data.updates
            for k in (
                "server_name", "server_description", "server_password",
                "admin_password", "max_players",
            )
        )

        if server.server_type == "native":
            if env_changed or ports_changed or identity_changed:
                # Rewrite config files
                NativeServerService.write_palworld_settings(server)
                # If running, restart
                if server.pid:
                    status = NativeServerService.get_process_status(server.pid)
                    if status.get("running"):
                        await NativeServerService.stop_server(server)
                        new_pid = NativeServerService.start_server(server)
                        if new_pid:
                            ServerDBService.update_server(server.id, {"pid": new_pid})
                            server = ServerDBService.get_server(server.id)
                logger.info("Updated native server %s config", server.name)
        else:
            if env_changed or ports_changed or identity_changed:
                # Recreate container with new settings
                DockerService.stop_server(old_server.container_name)
                DockerService.remove_server(old_server.container_name, remove_volumes=False)
                DockerService.create_server(server)
                logger.info("Recreated container %s with updated settings", server.container_name)

        result = _server_to_dict(server)
        result["status"] = _get_server_status(server)
        response = build_response(MessageType.UPDATE_SERVER, result)
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error updating server: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to update server: {e}"})
        await ws.send_json(response)


async def delete_server_handler(message: DeleteServerMessage, ws: WebSocket):
    try:
        server = ServerDBService.get_server(message.data.server_id)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        if server.server_type == "native":
            # Stop process if running
            if server.pid:
                await NativeServerService.stop_server(server)
            # Don't remove server files by default for native servers
            NativeServerService.remove_server(server.install_path, remove_data=False)
        else:
            # Stop and remove container
            DockerService.stop_server(server.container_name)
            DockerService.remove_server(server.container_name, remove_volumes=True)

        # Delete from DB
        ServerDBService.delete_server(server.id)

        response = build_response(MessageType.DELETE_SERVER, {"server_id": server.id})
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error deleting server: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to delete server: {e}"})
        await ws.send_json(response)


async def start_server_handler(message: StartServerMessage, ws: WebSocket):
    try:
        server = ServerDBService.get_server(message.data.server_id)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        if server.server_type == "native":
            pid = NativeServerService.start_server(server)
            success = pid is not None
            if pid:
                ServerDBService.update_server(server.id, {"pid": pid})
            status = NativeServerService.get_process_status(pid)
        else:
            success = DockerService.start_server(server.container_name)
            status = DockerService.get_container_status(server.container_name)

        response = build_response(
            MessageType.SERVER_STATUS_UPDATE,
            {"server_id": server.id, "status": status, "success": success},
        )
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error starting server: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to start server: {e}"})
        await ws.send_json(response)


async def stop_server_handler(message: StopServerMessage, ws: WebSocket):
    try:
        server = ServerDBService.get_server(message.data.server_id)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        if server.server_type == "native":
            success = await NativeServerService.stop_server(server)
            if success:
                ServerDBService.update_server(server.id, {"pid": None})
            status = NativeServerService.get_process_status(None)
        else:
            success = DockerService.stop_server(server.container_name)
            status = DockerService.get_container_status(server.container_name)

        response = build_response(
            MessageType.SERVER_STATUS_UPDATE,
            {"server_id": server.id, "status": status, "success": success},
        )
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error stopping server: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to stop server: {e}"})
        await ws.send_json(response)


async def server_api_call_handler(message: ServerApiCallMessage, ws: WebSocket):
    try:
        server = ServerDBService.get_server(message.data.server_id)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        result = await DockerService.rest_api_call(
            host="127.0.0.1",
            port=server.rest_api_port,
            admin_password=server.admin_password,
            endpoint=message.data.endpoint,
            method=message.data.method,
            data=message.data.payload,
        )

        response = build_response(
            MessageType.SERVER_API_RESPONSE,
            {
                "server_id": server.id,
                "endpoint": message.data.endpoint,
                "result": result,
            },
        )
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error calling server API: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"API call failed: {e}"})
        await ws.send_json(response)


async def list_server_mods_handler(message: ListServerModsMessage, ws: WebSocket):
    try:
        server = ServerDBService.get_server(message.data.server_id)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        mods = DockerService.list_mods(server.mods_path)

        # Also list logic mods
        if os.path.isdir(server.logicmods_path):
            for entry in os.listdir(server.logicmods_path):
                if entry.endswith(".pak"):
                    mods.append({
                        "mod_name": entry,
                        "mod_type": "logic",
                        "enabled": True,
                    })

        # Also list native/proxy DLL mods
        mods.extend(DockerService.list_native_mods(server.nativemods_path))

        response = build_response(
            MessageType.LIST_SERVER_MODS,
            {"server_id": server.id, "mods": mods},
        )
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error listing server mods: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to list mods: {e}"})
        await ws.send_json(response)


async def toggle_server_mod_handler(message: ToggleServerModMessage, ws: WebSocket):
    try:
        server = ServerDBService.get_server(message.data.server_id)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        DockerService.set_mod_enabled(server.mods_path, message.data.mod_name, message.data.enabled)

        response = build_response(
            MessageType.TOGGLE_SERVER_MOD,
            {
                "server_id": server.id,
                "mod_name": message.data.mod_name,
                "enabled": message.data.enabled,
            },
        )
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error toggling mod: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to toggle mod: {e}"})
        await ws.send_json(response)


async def install_server_mod_handler(message: InstallServerModMessage, ws: WebSocket):
    try:
        server = ServerDBService.get_server(message.data.server_id)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        mod_type = message.data.mod_type
        if mod_type == "native":
            success = DockerService.install_native_mod(
                server.nativemods_path, message.data.mod_name, message.data.mod_data
            )
        else:
            target_path = server.mods_path if mod_type == "ue4ss" else server.logicmods_path
            success = DockerService.install_mod(
                target_path, message.data.mod_name, message.data.mod_data, mod_type
            )

        response = build_response(
            MessageType.INSTALL_SERVER_MOD,
            {"server_id": server.id, "mod_name": message.data.mod_name, "success": success},
        )
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error installing mod: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to install mod: {e}"})
        await ws.send_json(response)


async def load_server_save_handler(message: LoadServerSaveMessage, ws: WebSocket):
    try:
        server = ServerDBService.get_server(message.data.server_id)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        # Verify server is stopped
        status = _get_server_status(server)
        if status and status.get("running"):
            response = build_response(
                MessageType.ERROR,
                {"message": "Server must be stopped before loading saves. Please stop the server first."},
            )
            await ws.send_json(response)
            return

        # Find the save directory: saves/SaveGames/0/{world_guid}/
        save_games_path = os.path.join(server.saves_path, "SaveGames", "0")
        if not os.path.isdir(save_games_path):
            response = build_response(
                MessageType.ERROR,
                {"message": f"No save data found at {save_games_path}"},
            )
            await ws.send_json(response)
            return

        # Find world directories
        world_dirs = [
            d for d in os.listdir(save_games_path)
            if os.path.isdir(os.path.join(save_games_path, d))
        ]

        if not world_dirs:
            response = build_response(
                MessageType.ERROR,
                {"message": "No world saves found in server save directory"},
            )
            await ws.send_json(response)
            return

        # Use the first (usually only) world directory
        world_dir = os.path.join(save_games_path, world_dirs[0])
        level_sav_path = os.path.join(world_dir, "Level.sav")

        if not os.path.exists(level_sav_path):
            response = build_response(
                MessageType.ERROR,
                {"message": "Level.sav not found in save directory"},
            )
            await ws.send_json(response)
            return

        # Use existing save loading pipeline
        validation = FileManager.validate_steam_save_directory(level_sav_path)
        if not validation.valid:
            response = build_response(MessageType.ERROR, {"message": validation.error})
            await ws.send_json(response)
            return

        app_state = get_app_state()

        with open(validation.level_sav, "rb") as f:
            level_sav = f.read()

        level_meta = None
        if validation.level_meta:
            with open(validation.level_meta, "rb") as f:
                level_meta = f.read()

        player_file_refs = FileManager.get_player_save_paths(validation.players_dir)

        await app_state.process_save_files(
            level_sav_path,
            level_sav,
            level_meta,
            player_file_refs,
            ws_callback=lambda msg: ws.send_json(
                build_response(MessageType.PROGRESS_MESSAGE, msg)
            ),
            local=True,
            gps_file_path=validation.global_pal_storage_sav,
        )

        # Update settings save_dir to point to this server's save
        app_state.settings.save_dir = world_dir

        data = {
            "level": validation.level_sav,
            "players": [str(p) for p in player_file_refs],
            "world_name": app_state.save_file.world_name,
            "type": "steam",
            "size": app_state.save_file.size,
            "has_gps": validation.global_pal_storage_sav is not None,
            "server_id": server.id,
            "server_name": server.name,
        }

        response = build_response(MessageType.LOADED_SAVE_FILES, data)
        await ws.send_json(response)

        response = build_response(
            MessageType.GET_PLAYER_SUMMARIES, app_state.player_summaries
        )
        await ws.send_json(response)

        response = build_response(
            MessageType.GET_GUILD_SUMMARIES, app_state.guild_summaries
        )
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error loading server save: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to load server save: {e}"})
        await ws.send_json(response)


async def get_server_stats_handler(message: GetServerStatsMessage, ws: WebSocket):
    try:
        server = ServerDBService.get_server(message.data.server_id)
        if not server:
            response = build_response(MessageType.ERROR, {"message": "Server not found"})
            await ws.send_json(response)
            return

        if server.server_type == "native":
            stats = NativeServerService.get_process_stats(server.pid)
        else:
            stats = DockerService.get_container_stats(server.container_name)

        response = build_response(
            MessageType.GET_SERVER_STATS,
            {"server_id": server.id, "stats": stats},
        )
        await ws.send_json(response)
    except Exception as e:
        logger.exception("Error getting server stats: %s", e)
        response = build_response(MessageType.ERROR, {"message": f"Failed to get server stats: {e}"})
        await ws.send_json(response)
