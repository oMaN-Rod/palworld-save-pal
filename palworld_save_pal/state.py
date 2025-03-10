import threading
from typing import Dict, Optional
from uuid import UUID
from pydantic import BaseModel, ConfigDict, Field
from webview import Window

from palworld_save_pal.editor.settings import Settings
from palworld_save_pal.game.guild import Guild
from palworld_save_pal.game.player import Player
from palworld_save_pal.game.save_file import SaveFile, SaveType
from palworld_save_pal.server_thread import ServerThread
from palworld_save_pal.utils.file_manager import GamepassSaveData
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)
settings_json = JsonManager("data/json/settings.json")


class AppState(BaseModel):
    save_file: Optional[SaveFile] = None
    save_type: SaveType = SaveType.STEAM
    players: Dict[UUID, Player] = Field(default_factory=dict)
    guilds: Dict[UUID, Guild] = Field(default_factory=dict)
    local: bool = False
    settings: Settings = Field(default_factory=Settings)
    terminate_flag: threading.Event = threading.Event()
    server_instance: Optional[ServerThread] = None
    webview_window: Optional[Window] = None
    gamepass_index_path: Optional[str] = None
    gamepass_saves: Dict[str, GamepassSaveData] = {}
    selected_gamepass_save: Optional[GamepassSaveData] = None

    model_config = ConfigDict(arbitrary_types_allowed=True)

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
        self.save_file = await SaveFile(name=sav_id).load_sav_files(
            level_sav, player_savs, level_meta, ws_callback
        )
        await ws_callback("Files loaded, getting players...")
        self.players = self.save_file.get_players()
        self.guilds = self.save_file.get_guilds()

    def select_gamepass_save(self, save_id: str) -> Optional[GamepassSaveData]:
        gamepass_save = self.gamepass_saves.get(save_id)
        self.selected_gamepass_save = gamepass_save
        return gamepass_save


app_state = AppState()


def get_app_state() -> AppState:
    return app_state
