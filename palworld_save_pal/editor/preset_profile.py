from typing import List, Optional
from pydantic import BaseModel

from palworld_save_pal.game.item_container_slot import ItemContainerSlot


class PresetProfile(BaseModel):
    name: str
    type: str
    skills: Optional[List[str]] = None
    common_container: Optional[List[ItemContainerSlot]] = None
    essential_container: Optional[List[ItemContainerSlot]] = None
    weapon_load_out_container: Optional[List[ItemContainerSlot]] = None
    player_equipment_armor_container: Optional[List[ItemContainerSlot]] = None
    food_equip_container: Optional[List[ItemContainerSlot]] = None
