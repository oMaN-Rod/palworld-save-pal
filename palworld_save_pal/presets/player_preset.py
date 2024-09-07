from typing import List
from pydantic import BaseModel, Field

from palworld_save_pal.save_file.item_container_slot import ItemContainerSlot


class PlayerPreset(BaseModel):
    name: str
    common_container: List[ItemContainerSlot] = Field(default_factory=list)
    essential_container: List[ItemContainerSlot] = Field(default_factory=list)
    weapon_load_out_container: List[ItemContainerSlot] = Field(default_factory=list)
    player_equipment_armor_container: List[ItemContainerSlot] = Field(
        default_factory=list
    )
    food_equip_container: List[ItemContainerSlot] = Field(default_factory=list)
