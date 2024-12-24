from typing import Dict, Optional
from uuid import UUID
from pydantic import BaseModel, Field

from palworld_save_pal.editor.settings import Settings
from palworld_save_pal.game.player import Player
from palworld_save_pal.game.save_file import SaveFile, SaveType
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)
settings_json = JsonManager("data/json/settings.json")


class AppState(BaseModel):
    save_file: Optional[SaveFile] = None
    save_type: SaveType = SaveType.STEAM
    players: Dict[UUID, Player] = Field(default_factory=dict)
    local: bool = False
    settings: Settings = Field(default_factory=lambda: load_settings())

    async def process_save_files(
        self,
        sav_id: str,
        level_sav: bytes,
        level_meta: Optional[bytes],
        player_savs: Dict[str, bytes],
        ws_callback=None,
        local=False,
        save_type: SaveType = SaveType.STEAM,
    ):
        logger.info("Processing save files for %s=>%s %s", sav_id, save_type, local)
        self.local = local
        self.save_type = save_type
        await ws_callback(f"Loading level.sav and {len(player_savs)} players...")
        self.save_file = SaveFile(name=sav_id).load_sav_files(
            level_sav, player_savs, level_meta
        )
        await ws_callback("Files loaded, getting players...")
        self.players = self.save_file.get_players()

    def update_settings(self, new_settings: Settings) -> None:
        """Update settings and save to file"""
        self.settings = new_settings
        settings_json.write(new_settings.dict())


def load_settings() -> Settings:
    """Load settings from JSON file or return defaults"""
    try:
        saved_settings = settings_json.read()
        if saved_settings:
            return Settings(**saved_settings)
    except Exception as e:
        logger.warning("Error loading settings: %s", e)

    # Return and save default settings if none exist
    default_settings = Settings(language="en")
    settings_json.write(default_settings.dict())
    return default_settings


app_state = AppState()


def get_app_state() -> AppState:
    return app_state
