from typing import Dict, Optional
from uuid import UUID
from pydantic import BaseModel, Field

from palworld_save_pal.pals.models import Player
from palworld_save_pal.save_file.save_file import SaveFile
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class AppState(BaseModel):
    save_file: Optional[SaveFile] = None
    players: Dict[UUID, Player] = Field(default_factory=dict)

    async def process_save_file(self, data: bytes, ws_callback=None):
        logger.info("Processing save file")
        await ws_callback("Loading GVAS...")
        self.save_file = SaveFile(name="Level.sav").load_gvas(data)
        await ws_callback("GVAS loaded, getting players...")
        self.players = self.save_file.get_players()


app_state = AppState()


def get_app_state() -> AppState:
    return app_state
