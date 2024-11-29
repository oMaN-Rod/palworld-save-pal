import os
from pathlib import Path
from typing import Dict, Optional
import uuid
from pydantic import BaseModel, ConfigDict
import webview
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

steam_root = os.path.join(os.getenv("LOCALAPPDATA"), "Pal", "Saved", "SaveGames")


class FileValidationResult(BaseModel):
    valid: bool
    level_sav: Optional[str] = None
    players_dir: Optional[str] = None
    error: Optional[str] = None

    model_config = ConfigDict(arbitrary_types_allowed=True)


class FileManager:
    @staticmethod
    def validate_steam_save_directory(
        save_path: str,
    ) -> FileValidationResult:
        save_dir = Path(os.path.dirname(save_path))
        level_sav = save_dir / "Level.sav"
        players_dir = save_dir / "Players"

        if not level_sav.exists():
            return FileValidationResult(
                valid=False,
                level_sav=None,
                players_dir=None,
                error="Level.sav file not found in the selected directory.",
            )

        if not players_dir.exists() or not players_dir.is_dir():
            return FileValidationResult(
                valid=False,
                level_sav=None,
                players_dir=None,
                error="Players directory not found in the selected directory.",
            )

        # Check if Players directory contains any .sav files
        player_saves = list(players_dir.glob("*.sav"))
        if not player_saves:
            return FileValidationResult(
                valid=False,
                level_sav=None,
                players_dir=None,
                error="No player save files found in the Players directory.",
            )

        return FileValidationResult(
            valid=True,
            level_sav=str(level_sav),
            players_dir=str(players_dir),
            error=None,
        )

    @staticmethod
    def open_file_dialog(window: webview.Window) -> Optional[str]:
        file_types = ("Sav Files (*.sav)", "All files (*.*)")
        file_path = steam_root
        result = window.create_file_dialog(
            webview.OPEN_DIALOG,
            directory=file_path,
            allow_multiple=False,
            file_types=file_types,
        )

        if result and len(result) > 0:
            file_path = result[0]
            if os.path.basename(file_path) == "Level.sav":
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
