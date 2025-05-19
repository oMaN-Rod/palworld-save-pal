from typing import List, Optional
from uuid import UUID
from pydantic import BaseModel

from palworld_save_pal.game.enum import PalGender


class DynamicItemDTO(BaseModel):
    local_id: UUID
    character_id: Optional[str] = None
    durability: Optional[float] = None
    passive_skill_list: Optional[List[str]] = None
    remaining_bullets: Optional[int] = None
    type: Optional[str] = None
    egg_character_id: Optional[str] = None
    gender: Optional[PalGender] = None
    talent_hp: Optional[int] = None
    talent_shot: Optional[int] = None
    talent_defense: Optional[int] = None
    learned_skills: Optional[List[str]] = None
    active_skills: Optional[List[str]] = None
    passive_skills: Optional[List[str]] = None
