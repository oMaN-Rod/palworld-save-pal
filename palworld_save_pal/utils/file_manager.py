from datetime import datetime, timezone
import os
from pathlib import Path
import platform
from typing import Dict, Optional
import uuid
from pydantic import BaseModel, ConfigDict
import webview
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.game.player import PlayerGvasFiles
from palworld_save_pal.game.save_file import SaveFile
from palworld_save_pal.utils.gamepass.container_types import (
    Container,
    ContainerFileList,
    ContainerIndex,
)
from palworld_save_pal.utils.gamepass.container_utils import read_container_index
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

FILETIME_EPOCH = datetime(1601, 1, 1, tzinfo=timezone.utc)
STEAM_ROOT = (
    os.path.join(os.getenv("LOCALAPPDATA"), "Pal", "Saved", "SaveGames")
    if os.name == "nt"
    else (
        os.path.join(
            "/System/Volumes/Data/Users",
            os.getenv("USER"),
            "Library/Containers/com.pocketpair.palworld.mac/Data/Library/Application Support/Epic/Pal/Saved/SaveGames",
        )
        if platform.system() == "Darwin"
        else "~"
    )
)
GAMEPASS_ROOT = (
    os.path.join(
        os.getenv("LOCALAPPDATA"),
        "Packages",
        "PocketpairInc.Palworld_ad4psfrxyesvt",
        "SystemAppData",
        "wgs",
    )
    if os.name == "nt"
    else "~"
)


class GamepassSaveData(BaseModel):
    save_id: str
    world_name: str
    player_count: int


class FileValidationResult(BaseModel):
    valid: bool
    level_sav: Optional[str] = None
    level_meta: Optional[str] = None
    players_dir: Optional[str] = None
    error: Optional[str] = None
    gamepass_saves: Optional[Dict[str, GamepassSaveData]] = None
    global_pal_storage_sav: Optional[str] = None

    model_config = ConfigDict(arbitrary_types_allowed=True)


class FileManager:
    @staticmethod
    def validate_steam_save_directory(
        save_path: str,
    ) -> FileValidationResult:
        save_dir = Path(os.path.dirname(save_path))
        level_sav = save_dir / "Level.sav"
        level_meta = save_dir / "LevelMeta.sav"
        players_dir = save_dir / "Players"
        parent_dir = save_dir.parent
        global_pal_storage_sav = parent_dir / "GlobalPalStorage.sav"

        if not level_sav.exists():
            return FileValidationResult(
                valid=False,
                level_sav=None,
                level_meta=None,
                players_dir=None,
                error="Level.sav file not found in the selected directory.",
            )

        if not players_dir.exists() or not players_dir.is_dir():
            return FileValidationResult(
                valid=False,
                error="Players directory not found in the selected directory.",
            )

        if not level_meta.exists():
            level_meta = None

        player_saves = list(players_dir.glob("*.sav"))
        if not player_saves:
            return FileValidationResult(
                valid=False,
                error="No player save files found in the Players directory.",
            )

        if not global_pal_storage_sav.exists():
            global_pal_storage_sav = None

        return FileValidationResult(
            valid=True,
            level_sav=str(level_sav),
            level_meta=str(level_meta) if level_meta else None,
            players_dir=str(players_dir),
            error=None,
            global_pal_storage_sav=str(global_pal_storage_sav)
            if global_pal_storage_sav
            else None,
        )

    @staticmethod
    def open_file_dialog(
        save_type: str, window: webview.Window, save_dir: str = None
    ) -> Optional[str]:
        app_dir = Path(
            __file__
        ).parent.parent.parent.resolve()  # Get the app's root directory
        logger.debug("Application directory: %s", app_dir)

        initial_dir = STEAM_ROOT
        if save_type == "steam":
            file_types = ("Sav Files (*.sav)", "All files (*.*)")
        elif save_type == "local_data":
            file_types = ("Sav Files (*.sav)", "All files (*.*)")
        else:
            file_types = ("Container Index Files (*.index)", "All files (*.*)")
            initial_dir = GAMEPASS_ROOT

        result = window.create_file_dialog(
            webview.OPEN_DIALOG,
            directory=initial_dir if save_dir is None else save_dir,
            allow_multiple=False,
            file_types=file_types,
        )

        if result and len(result) > 0:
            selected_path_str = result[0]
            selected_path = Path(selected_path_str).resolve()
            logger.debug("Selected path: %s", selected_path)

            # Check if the selected path is within the application directory
            if str(selected_path).startswith(str(app_dir)):
                logger.warning(
                    "Selected path %s is inside the application directory %s. Ignoring.",
                    selected_path,
                    app_dir,
                )
                raise ValueError(
                    "Selected path is inside the PSP application directory. Please move your save files outside of the application directory."
                )

            file_name = os.path.basename(selected_path_str)
            if (
                (file_name == "Level.sav" and save_type == "steam")
                or (file_name == "containers.index" and save_type == "gamepass")
                or (file_name == "LocalData.sav" and save_type == "local_data")
            ):
                return selected_path_str
            else:
                logger.warning(
                    "Selected file %s does not match expected type for %s save.",
                    file_name,
                    save_type,
                )
                raise ValueError(
                    f"Selected file {file_name} does not match expected type for {save_type} save. Please select a valid save file."
                )
        return None

    @staticmethod
    def get_player_saves(players_dir: str) -> Dict[uuid.UUID, Dict[str, bytes]]:
        player_saves: Dict[uuid.UUID, Dict[str, bytes]] = {}
        players_path = Path(players_dir)

        for save_file in players_path.glob("*.sav"):
            dps = False
            try:
                player_id = save_file.stem
                if "_dps" in player_id:
                    player_id = player_id.replace("_dps", "")
                    dps = True

                logger.debug("Reading player save: %s, uuid: %s", save_file, player_id)
                player_uuid = uuid.UUID(player_id)

                if player_uuid not in player_saves:
                    player_saves[player_uuid] = {}

                with open(save_file, "rb") as f:
                    save_type = "dps" if dps else "sav"
                    player_saves[player_uuid][save_type] = f.read()

            except:
                logger.error("Failed to read player save: %s", save_file, exc_info=True)
                continue

        return player_saves

    @staticmethod
    def get_player_save_paths(players_dir: str) -> Dict[uuid.UUID, Dict[str, str]]:
        player_save_paths: Dict[uuid.UUID, Dict[str, str]] = {}
        players_path = Path(players_dir)

        for save_file in players_path.glob("*.sav"):
            dps = False
            try:
                player_id = save_file.stem
                if "_dps" in player_id:
                    player_id = player_id.replace("_dps", "")
                    dps = True

                logger.debug(
                    "Found player save path: %s, uuid: %s", save_file, player_id
                )
                player_uuid = uuid.UUID(player_id)

                if player_uuid not in player_save_paths:
                    player_save_paths[player_uuid] = {}

                save_type = "dps" if dps else "sav"
                player_save_paths[player_uuid][save_type] = str(save_file)

            except:
                logger.error(
                    "Failed to parse player save path: %s", save_file, exc_info=True
                )
                continue

        return player_save_paths

    @staticmethod
    def read_level_meta(data: bytes) -> Optional[str]:
        level_meta = SaveFile().load_level_meta(data)
        world_name = PalObjects.get_nested(
            level_meta.properties, "SaveData", "value", "WorldName", "value"
        )
        return world_name if world_name else "Unknown World"

    @staticmethod
    def parse_gamepass_saves(containers_path: Path) -> Dict[str, GamepassSaveData]:
        logger.debug("Parsing GamePass saves using path: %s", containers_path)
        saves: Dict[str, GamepassSaveData] = {}

        container_index: ContainerIndex = read_container_index(containers_path)
        recent_containers: Dict[str, Dict[str, Container]] = {}

        for container in container_index.containers:
            parts = container.container_name.split("-")
            if len(parts) < 2:
                continue
            save_id = parts[0]
            if save_id not in recent_containers:
                recent_containers[save_id] = container_index.get_save_containers(
                    save_id
                )

        for save_id, container in recent_containers.items():
            level_meta_container = container.get("LevelMeta", None)
            if level_meta_container is None:
                continue

            level_meta_dir = os.path.join(
                containers_path,
                level_meta_container.container_uuid.bytes_le.hex().upper(),
            )
            logger.debug("Reading container files from: %s", level_meta_dir)
            world_name = "Unknown World"

            valid = False
            for filename in os.listdir(level_meta_dir):
                if filename.startswith("container."):
                    logger.debug("Reading container file: %s", filename)
                    with open(os.path.join(level_meta_dir, filename), "rb") as f:
                        file_list = ContainerFileList.from_stream(f)
                        if len(file_list.files) > 0:
                            valid = True
                            world_name = FileManager.read_level_meta(
                                file_list.files[0].data
                            )
                    break

            if not valid:
                continue

            player_count = len(
                [
                    c
                    for c in container.values()
                    if "Player" in c.container_name and "_dps" not in c.container_name
                ]
            )
            logger.debug(
                "Found save: %s with world name: %s and %s players",
                save_id,
                world_name,
                player_count,
            )
            saves[save_id] = GamepassSaveData(
                save_id=save_id,
                world_name=world_name,
                player_count=player_count,
            )

        return saves

    @staticmethod
    def validate_gamepass_directory(file_path: str) -> FileValidationResult:
        logger.debug("Validating GamePass save directory: %s", file_path)
        containers_path = Path(os.path.dirname(file_path))
        containers_index = containers_path / "containers.index"

        if not containers_index.exists():
            return FileValidationResult(
                valid=False,
                error="containers.index file not found in the selected directory.",
            )

        saves = FileManager.parse_gamepass_saves(containers_path)
        if not saves:
            return FileValidationResult(
                valid=False,
                error="No valid Palworld saves found in the selected directory.",
            )

        return FileValidationResult(valid=True, gamepass_saves=saves, error=None)
