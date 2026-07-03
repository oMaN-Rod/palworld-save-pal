from datetime import datetime
from typing import Optional
from uuid import UUID
from pydantic import BaseModel


class PlayerSummary(BaseModel):
    uid: UUID
    nickname: str
    level: Optional[int] = None
    guild_id: Optional[UUID] = None
    pal_count: int = 0
    last_online_time: Optional[datetime] = None
    loaded: bool = False


class GuildSummary(BaseModel):
    id: UUID
    name: str
    admin_player_uid: Optional[UUID] = None
    player_count: int = 0
    base_count: int = 0
    level: Optional[int] = None
    pal_count: int = 0
    loaded: bool = False


class SaveFileSummary(BaseModel):
    level_path: str
    world_name: str
    save_type: str
    size: int
    player_count: int
    guild_count: int


class PalSummary(BaseModel):
    instance_id: UUID
    character_id: str
    character_key: str
    nickname: Optional[str] = None
    owner_uid: Optional[UUID] = None
    owner_name: Optional[str] = None
    guild_id: Optional[UUID] = None
    base_id: Optional[UUID] = None
    gender: Optional[str] = None
    level: int = 1
    hp: int = 0
    stomach: float = 150.0
    rank: int = 1
    exp: int = 0
    talent_hp: int = 0
    talent_shot: int = 0
    talent_defense: int = 0
    rank_hp: int = 0
    rank_attack: int = 0
    rank_defense: int = 0
    rank_craftspeed: int = 0
