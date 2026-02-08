import threading
from typing import Any, Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel, ConfigDict, Field
from webview import Window

from palworld_save_pal.dto.summary import PlayerSummary, GuildSummary
from palworld_save_pal.editor.settings import Settings
from palworld_save_pal.game.guild import Guild
from palworld_save_pal.game.pal import Pal
from palworld_save_pal.game.player import Player
from palworld_save_pal.game.save_file import SaveFile, SaveType
from palworld_save_pal.server_thread import ServerThread
from palworld_save_pal.utils.file_manager import GamepassSaveData
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


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
    gps: Optional[Dict[int, Pal]] = Field(default_factory=dict)

    player_summaries: Dict[UUID, PlayerSummary] = Field(default_factory=dict)
    guild_summaries: Dict[UUID, GuildSummary] = Field(default_factory=dict)
    gps_file_path: Optional[str] = None
    gps_loaded: bool = False

    model_config = ConfigDict(arbitrary_types_allowed=True)

    def select_gamepass_save(self, save_id: str) -> Optional[GamepassSaveData]:
        gamepass_save = self.gamepass_saves.get(save_id)
        self.selected_gamepass_save = gamepass_save
        return gamepass_save

    async def process_save_files(
        self,
        sav_id: str,
        level_sav: bytes,
        level_meta: Optional[bytes],
        player_file_refs: Dict[UUID, Dict[str, Any]],
        ws_callback=None,
        local=False,
        save_type: SaveType = SaveType.STEAM,
        gps_file_path: Optional[str] = None,
    ):
        logger.info("Processing save files for %s=>%s %s", sav_id, save_type, local)
        self.local = local
        self.save_type = save_type

        await ws_callback("Loading Level.sav...")

        self.save_file = await SaveFile(level_sav_path=sav_id).load_sav_files(
            level_sav, player_file_refs, level_meta, ws_callback
        )

        self.player_summaries = self.save_file.get_player_summaries()
        self.guild_summaries = self.save_file.get_guild_summaries()

        self.players = {}
        self.guilds = {}

        self.gps_file_path = gps_file_path
        self.gps_loaded = False
        self.gps = {}

        logger.info(
            "Load complete: %d player summaries, %d guild summaries, GPS deferred: %s",
            len(self.player_summaries),
            len(self.guild_summaries),
            gps_file_path is not None,
        )

    async def get_player_details(
        self, player_id: UUID, ws_callback=None
    ) -> Optional[Player]:
        if not self.save_file:
            logger.error("No save file loaded")
            return None

        if player_id in self.players:
            logger.info(f"Player {player_id} already loaded, returning cached")
            return self.players[player_id]

        player = await self.save_file.load_player_on_demand(player_id, ws_callback)

        if player:
            self.players[player_id] = player

            if player_id in self.player_summaries:
                self.player_summaries[player_id].loaded = True

            logger.info(
                f"Player {player_id} loaded on demand with {len(player.pals)} pals"
            )

        return player

    async def get_guild_details(
        self, guild_id: UUID, ws_callback=None
    ) -> Optional[Guild]:
        if not self.save_file:
            logger.error("No save file loaded")
            return None

        if guild_id in self.guilds:
            logger.info(f"Guild {guild_id} already loaded, returning cached")
            return self.guilds[guild_id]

        guild = self.save_file._load_guild_by_id(guild_id)

        if guild:
            self.guilds[guild_id] = guild

            if guild_id in self.guild_summaries:
                self.guild_summaries[guild_id].loaded = True

            logger.info(f"Guild {guild_id} loaded on demand")

        return guild

    def is_player_loaded(self, player_id: UUID) -> bool:
        return player_id in self.players

    def is_guild_loaded(self, guild_id: UUID) -> bool:
        return guild_id in self.guilds

    async def load_gps_on_demand(self, ws_callback=None) -> Optional[Dict[int, Pal]]:
        if not self.save_file:
            logger.error("No save file loaded")
            return None

        if self.gps_loaded and self.gps:
            logger.info("GPS already loaded, returning cached data")
            return self.gps

        if not self.gps_file_path:
            logger.info("No GPS file available for this save")
            return None

        if ws_callback:
            await ws_callback("Loading Global Pal Storage...")

        try:
            with open(self.gps_file_path, "rb") as f:
                gps_bytes = f.read()

            self.gps = self.save_file.load_gps(gps_bytes)
            self.gps_loaded = True

            logger.info(
                f"GPS loaded on demand with {len(self.gps) if self.gps else 0} pals"
            )
            return self.gps
        except Exception as e:
            logger.error(f"Failed to load GPS: {e}")
            return None

    def has_gps_available(self) -> bool:
        return self.gps_loaded or self.gps_file_path is not None


app_state = AppState()


def get_app_state() -> AppState:
    return app_state
