from typing import Dict, Optional
from uuid import UUID
from pydantic import BaseModel, Field

from palworld_save_pal.save_file.pal import Pal
from palworld_save_pal.save_file.item_container import ItemContainer


class Player(BaseModel):
    uid: UUID
    nickname: str
    level: int
    pals: Optional[Dict[UUID, Pal]] = Field(default_factory=dict)
    common_container: Optional[ItemContainer] = Field(default=None)
    essential_container: Optional[ItemContainer] = Field(default=None)
    weapon_load_out_container: Optional[ItemContainer] = Field(default=None)
    player_equipment_armor_container: Optional[ItemContainer] = Field(default=None)
    food_equip_container: Optional[ItemContainer] = Field(default=None)
