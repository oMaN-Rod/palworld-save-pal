from typing import Optional
from uuid import UUID
from pydantic import BaseModel


class PlayerSummary(BaseModel):
    uid: UUID
    nickname: str
    level: Optional[int] = None
    guild_id: Optional[UUID] = None
    pal_count: int = 0
    loaded: bool = False


class GuildSummary(BaseModel):
    id: UUID
    name: str
    admin_player_uid: Optional[UUID] = None
    player_count: int = 0
    base_count: int = 0
    loaded: bool = False


class SaveFileSummary(BaseModel):
    level_path: str
    world_name: str
    save_type: str
    size: int
    player_count: int
    guild_count: int
