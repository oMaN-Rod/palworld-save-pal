from typing import Optional

from pydantic import BaseModel

from palworld_save_pal.save_file.dynamic_item import DynamicItem


class ItemContainerSlot(BaseModel):
    slot_index: int
    count: int
    static_id: Optional[str] = None
    dynamic_item: Optional[DynamicItem] = None
