from typing import Optional

from pydantic import BaseModel

from palworld_save_tools.archive import *

from palworld_save_pal.game.dynamic_item import DynamicItem


class ItemContainerSlot(BaseModel):
    slot_index: int
    count: int
    static_id: Optional[str] = None
    dynamic_item: Optional[DynamicItem] = None
