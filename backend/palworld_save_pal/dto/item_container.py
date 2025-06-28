from enum import Enum
from typing import List, Optional
from uuid import UUID
from pydantic import BaseModel

from palworld_save_pal.dto.item_container_slot import ItemContainerSlotDTO


class ItemContainerType(str, Enum):
    COMMON = "CommonContainer"
    ESSENTIAL = "EssentialContainer"
    WEAPON = "WeaponLoadOutContainer"
    ARMOR = "PlayerEquipArmorContainer"
    FOOD = "FoodEquipContainer"
    BASE = "BaseContainer"
    GUILD = "GuildChest"


class ItemContainerDTO(BaseModel):
    id: UUID
    type: ItemContainerType
    slots: List[ItemContainerSlotDTO] = []
    key: Optional[str] = None
    slot_num: int = 0
