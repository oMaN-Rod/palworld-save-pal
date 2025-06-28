from typing import Optional
from pydantic import BaseModel

from palworld_save_pal.dto.dynamic_item import DynamicItemDTO


class ItemContainerSlotDTO(BaseModel):
    slot_index: int
    count: int
    static_id: Optional[str] = None
    dynamic_item: Optional[DynamicItemDTO] = None
