from typing import Dict, List, Optional
from pydantic import BaseModel

from palworld_save_pal.editor.preset_profile import StorageContainerPreset
from palworld_save_pal.game.enum import PalGender


class PalPresetDTO(BaseModel):
    lock: bool
    lock_element: bool
    element: Optional[str] = None
    character_id: Optional[str] = None
    is_lucky: Optional[bool] = None
    is_boss: Optional[bool] = None
    gender: Optional[PalGender] = None
    rank_hp: Optional[int] = None
    rank_attack: Optional[int] = None
    rank_defense: Optional[int] = None
    rank_craftspeed: Optional[int] = None
    talent_hp: Optional[int] = None
    talent_shot: Optional[int] = None
    talent_defense: Optional[int] = None
    rank: Optional[int] = None
    level: Optional[int] = None
    exp: Optional[int] = None
    learned_skills: Optional[List[str]] = None
    active_skills: Optional[List[str]] = None
    passive_skills: Optional[List[str]] = None
    sanity: Optional[float] = None
    work_suitability: Optional[Dict[str, int]] = None


class PresetProfileDTO(BaseModel):
    name: str
    type: str
    skills: Optional[List[str]] = None

    common_container: Optional[List[dict]] = None
    essential_container: Optional[List[dict]] = None
    weapon_load_out_container: Optional[List[dict]] = None
    player_equipment_armor_container: Optional[List[dict]] = None
    food_equip_container: Optional[List[dict]] = None
    storage_container: Optional[StorageContainerPreset] = None

    pal_preset: Optional[PalPresetDTO] = None
