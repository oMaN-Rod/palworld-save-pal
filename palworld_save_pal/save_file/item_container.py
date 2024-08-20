from typing import List, Optional
from uuid import UUID
from pydantic import BaseModel, Field

from palworld_save_pal.save_file.dynamic_item import DynamicItem
from palworld_save_pal.save_file.encoders import custom_uuid_encoder


class ContainerSlot(BaseModel):
    slot_index: int
    count: int
    static_id: Optional[str] = None
    dynamic_item: Optional[DynamicItem] = None


class ItemContainer(BaseModel):
    id: UUID = Field(..., json_encoder=custom_uuid_encoder)
    type: str
    slots: List[ContainerSlot] = Field(default_factory=list)

    # pylint: disable=E1133
    def get_slot(self, slot_index: int) -> Optional[ContainerSlot]:
        return next(
            (slot for slot in self.slots if slot.slot_index == slot_index), None
        )
