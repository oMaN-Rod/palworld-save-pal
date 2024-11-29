from typing import Dict, Optional
from uuid import UUID
from pydantic import BaseModel, Field

from palworld_save_pal.game.player import Player
from palworld_save_pal.game.save_file import SaveFile, SaveType
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class AppState(BaseModel):
    save_file: Optional[SaveFile] = None
    save_type: SaveType = SaveType.STEAM
    players: Dict[UUID, Player] = Field(default_factory=dict)
    local: bool = False

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


app_state = AppState()


def get_app_state() -> AppState:
    return app_state
