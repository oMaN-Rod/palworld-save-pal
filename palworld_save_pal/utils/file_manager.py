from datetime import datetime, timedelta, timezone
import os
from pathlib import Path
import struct
from typing import Dict, List, Optional
import uuid
from pydantic import BaseModel, ConfigDict, Field
import webview
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.game.save_file import SaveFile
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

FILETIME_EPOCH = datetime(1601, 1, 1, tzinfo=timezone.utc)
STEAM_ROOT = (
    os.path.join(os.getenv("LOCALAPPDATA"), "Pal", "Saved", "SaveGames")
    if os.name == "nt"
    else None
)
GAMEPASS_ROOT = os.path.join(
    os.getenv("LOCALAPPDATA"),
    "Packages",
    "PocketpairInc.Palworld_ad4psfrxyesvt",
    "SystemAppData",
    "wgs",
)


class GamepassContainer(BaseModel):
    container_dir: Path
    container_file: Path
    file: Optional[Path]
    guid: str
    name: str


class GamepassSaveData(BaseModel):
    save_id: str
    world_name: str
    player_count: int
    containers: List[GamepassContainer] = Field(default_factory=list)


class FileValidationResult(BaseModel):
    valid: bool
    level_sav: Optional[str] = None
    level_meta: Optional[str] = None
    players_dir: Optional[str] = None
    error: Optional[str] = None
    gamepass_saves: Optional[Dict[str, GamepassSaveData]] = None

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

        # Check if Players directory contains any .sav files
        player_saves = list(players_dir.glob("*.sav"))
        if not player_saves:
            return FileValidationResult(
                valid=False,
                error="No player save files found in the Players directory.",
            )

        return FileValidationResult(
            valid=True,
            level_sav=str(level_sav),
            level_meta=str(level_meta) if level_meta else None,
            players_dir=str(players_dir),
            error=None,
        )

    @staticmethod
    def open_file_dialog(
        save_type: str, window: webview.Window, _: str
    ) -> Optional[str]:
        if save_type == "steam":
            file_types = ("Sav Files (*.sav)", "All files (*.*)")
            file_path = STEAM_ROOT
        else:
            file_types = ("Container Index Files (*.index)", "All files (*.*)")
            file_path = GAMEPASS_ROOT

        result = window.create_file_dialog(
            webview.OPEN_DIALOG,
            directory=file_path,
            allow_multiple=False,
            file_types=file_types,
        )

        if result and len(result) > 0:
            file_path = result[0]
            if (
                os.path.basename(file_path) == "Level.sav" and save_type == "steam"
            ) or (
                os.path.basename(file_path) == "containers.index"
                and save_type == "gamepass"
            ):
                return file_path
        return None

    @staticmethod
    def get_player_saves(players_dir: str) -> Dict[str, bytes]:
        player_saves = {}
        players_path = Path(players_dir)

        for save_file in players_path.glob("*.sav"):
            player_id = save_file.stem
            player_uuid = uuid.UUID(player_id)
            with open(save_file, "rb") as f:
                player_saves[player_uuid] = f.read()

        return player_saves

    @staticmethod
    def read_utf16_str(f, str_len: Optional[int] = None) -> str:
        if not str_len:
            str_len = struct.unpack("<i", f.read(4))[0]
        return f.read(str_len * 2).decode("utf-16").rstrip("\0")

    @staticmethod
    def read_filetime(f) -> datetime:
        filetime = struct.unpack("<Q", f.read(8))[0]
        filetime_seconds = filetime / 10_000_000
        return FILETIME_EPOCH + timedelta(seconds=filetime_seconds)

    @staticmethod
    def read_level_meta(file_path: Path) -> Optional[str]:
        logger.debug("Reading LevelMeta.sav: %s", file_path)
        with open(file_path, "rb") as f:
            data = SaveFile().load_level_meta(f.read())
            world_name = PalObjects.get_nested(
                data, "SaveData", "value", "WorldName", "value"
            )
            return world_name if world_name else "Unknown World"

    @staticmethod
    def read_gamepass_containers(containers_path: Path) -> Dict[str, GamepassContainer]:
        containers: Dict[str, GamepassContainer] = {}
        containers_dir = containers_path.parent

        with containers_path.open("rb") as f:
            f.read(4)  # version
            container_count = struct.unpack("<i", f.read(4))[0]
            FileManager.read_utf16_str(f)  # flag1
            FileManager.read_utf16_str(f)  # package name
            FileManager.read_filetime(f)  # mtime
            f.read(4)  # flag2
            FileManager.read_utf16_str(f)  # index uuid
            f.read(8)  # unknown

            for _ in range(container_count):
                container_name = FileManager.read_utf16_str(f)
                FileManager.read_utf16_str(f)  # container name
                FileManager.read_utf16_str(f)  # cloud id

                seq = struct.unpack("B", f.read(1))[0]
                f.read(4)  # flag

                container_guid = uuid.UUID(bytes_le=f.read(16))
                FileManager.read_filetime(f)  # mtime
                f.read(16)  # reserved and size

                container_path = containers_dir / container_guid.hex.upper()
                container_file = container_path / f"container.{seq}"

                if container_file.exists():
                    container_files = FileManager.read_container_files(container_file)
                    file = container_files.get("Data")
                    if container_guid.hex.upper() not in containers:
                        containers[container_guid.hex.upper()] = GamepassContainer(
                            container_dir=container_path,
                            container_file=container_file,
                            guid=container_guid.hex.upper(),
                            name=container_name,
                            file=file,
                        )

        return containers

    @staticmethod
    def read_container_files(container_path: Path) -> Dict[str, Path]:
        logger.debug("Reading container file: %s", container_path)
        files: Dict[str, Path] = {}
        container_dir = container_path.parent

        with container_path.open("rb") as f:
            f.read(4)
            file_count = int.from_bytes(f.read(4), "little")
            for _ in range(file_count):
                file_name = FileManager.read_utf16_str(f, 64)
                f.read(16)
                file_guid = uuid.UUID(bytes=f.read(16))

                file_path = container_dir / file_guid.bytes_le.hex().upper()
                if file_path.exists():
                    files[file_name] = file_path
        return files

    @staticmethod
    def parse_gamepass_saves(containers_path: Path) -> Dict[str, GamepassSaveData]:
        logger.debug("Parsing GamePass saves from containers.index")
        saves: Dict[str, GamepassSaveData] = {}

        containers = FileManager.read_gamepass_containers(containers_path)
        for container in containers.values():

            # Parse save folder structure from container name
            parts = container.name.split("-")
            if len(parts) < 2:
                continue
            save_id = parts[0]

            if save_id not in saves:
                saves[save_id] = GamepassSaveData(
                    save_id=save_id, world_name="Unknown", player_count=0, containers=[]
                )
                saves[save_id].containers.append(container)
            else:
                saves[save_id].containers.append(container)

            if parts[-1] == "LevelMeta":
                level_meta_path = container.file
                world_name = "Unknown"
                if level_meta_path and level_meta_path.exists():
                    world_name = (
                        FileManager.read_level_meta(level_meta_path) or world_name
                    )
                saves[save_id].world_name = world_name

            if parts[-2] == "Players":
                if "player_count" not in container:
                    saves[save_id].player_count = 0
                saves[save_id].player_count += 1

        return saves

    @staticmethod
    def validate_gamepass_directory(file_path: str) -> FileValidationResult:
        logger.debug("Validating GamePass save directory: %s", file_path)
        save_dir = Path(os.path.dirname(file_path))
        containers_index = save_dir / "containers.index"

        if not containers_index.exists():
            return FileValidationResult(
                valid=False,
                error="containers.index file not found in the selected directory.",
            )

        saves = FileManager.parse_gamepass_saves(containers_index)
        if not saves:
            return FileValidationResult(
                valid=False,
                error="No valid Palworld saves found in the selected directory.",
            )

        return FileValidationResult(valid=True, gamepass_saves=saves, error=None)
