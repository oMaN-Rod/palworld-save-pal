from typing import Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel

from palworld_save_pal.game.item_container_slot import ItemContainerSlot
from palworld_save_pal.game.pal_objects import PalGender, WorkSuitability


class StorageContainerPreset(BaseModel):
    key: str
    slots: List[ItemContainerSlot]


class PalPreset(BaseModel):
    lock: bool
    character_id: Optional[str]
    is_lucky: Optional[bool]
    is_boss: Optional[bool]
    gender: Optional[PalGender]
    rank_hp: Optional[int]
    rank_attack: Optional[int]
    rank_defense: Optional[int]
    rank_craftspeed: Optional[int]
    talent_hp: Optional[int]
    talent_shot: Optional[int]
    talent_defense: Optional[int]
    rank: Optional[int]
    level: Optional[int]
    exp: Optional[int]
    learned_skills: Optional[List[str]]
    active_skills: Optional[List[str]]
    passive_skills: Optional[List[str]]
    sanity: Optional[float]
    work_suitability: Optional[Dict[WorkSuitability, int]]


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
