from typing import Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel

from palworld_save_pal.game.item_container_slot import ItemContainerSlot
from palworld_save_pal.game.pal_objects import PalGender, WorkSuitability


class StorageContainerPreset(BaseModel):
    key: str
    slots: List[ItemContainerSlot]


class PalPreset(BaseModel):
    is_lucky: bool
    is_boss: bool
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
    learned_skills: List[str]
    active_skills: List[str]
    passive_skills: List[str]
    sanity: float
    work_suitability: Dict[WorkSuitability, int]


class PresetProfile(BaseModel):
    name: str
    type: str
    skills: Optional[List[str]] = None
    common_container: Optional[List[ItemContainerSlot]] = None
    essential_container: Optional[List[ItemContainerSlot]] = None
    weapon_load_out_container: Optional[List[ItemContainerSlot]] = None
    player_equipment_armor_container: Optional[List[ItemContainerSlot]] = None
    food_equip_container: Optional[List[ItemContainerSlot]] = None
    storage_container: Optional[StorageContainerPreset] = None
    pal_preset: Optional[PalPreset] = None
