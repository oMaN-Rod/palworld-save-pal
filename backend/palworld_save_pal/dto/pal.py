from typing import Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel

from palworld_save_pal.game.pal_objects import PalGender, WorkSuitability


class PalDTO(BaseModel):
    instance_id: UUID
    owner_uid: Optional[UUID]
    character_id: str
    is_lucky: Optional[bool]
    is_boss: Optional[bool]
    gender: PalGender
    rank_hp: int
    rank_attack: int
    rank_defense: int
    rank_craftspeed: int
    talent_hp: int
    talent_shot: int
    talent_defense: int
    rank: int
    level: int
    exp: int
    nickname: Optional[str]
    is_tower: bool
    storage_id: UUID
    stomach: float
    storage_slot: int
    learned_skills: List[str]
    active_skills: List[str]
    passive_skills: List[str]
    hp: int
    max_hp: int
    group_id: Optional[UUID]
    sanity: float
    work_suitability: Dict[WorkSuitability, int]
    is_sick: bool
