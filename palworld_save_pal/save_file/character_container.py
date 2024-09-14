from typing import Any, Dict, List, Optional
from uuid import UUID
import uuid
from pydantic import BaseModel, Field, PrivateAttr

from palworld_save_tools.archive import UUID as ArchiveUUID

from palworld_save_pal.save_file.encoders import custom_uuid_encoder
from palworld_save_pal.save_file.pal_objects import PalObjects
from palworld_save_pal.save_file.utils import are_equal_uuids
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class CharacterContainerSlot(BaseModel):
    slot_index: int
    pal_id: Optional[UUID] = None


class CharacterContainer(BaseModel):
    id: UUID = Field(..., json_encoder=custom_uuid_encoder)
    size: Optional[int] = 0
    slots: Optional[List[CharacterContainerSlot]] = Field(default_factory=list)

    _slots_data: Optional[List[Dict[str, Any]]] = PrivateAttr(default_factory=list)

    def __init__(self, character_container_save_data: Dict[str, Any] = None, **kwargs):
        super().__init__(**kwargs)
        if character_container_save_data is not None:
            self._get_items(character_container_save_data)

    def available_slots(self) -> bool:
        return len(self.slots) < self.size

    def add_pal(self, pal_id: UUID) -> Optional[int]:
        if not self.available_slots():
            logger.warning("Character container is full")
            return
        slot_idx = len(self.slots)
        logger.debug(
            "pal_id = %s, slot_idx = %s, container_id = %s", pal_id, slot_idx, self.id
        )
        new_container_slot_data = PalObjects.ContainerSlotData(
            slot_idx=slot_idx, instance_id=pal_id
        )
        self._slots_data.append(new_container_slot_data)
        self.slots.append(CharacterContainerSlot(slot_index=slot_idx, pal_id=pal_id))
        return slot_idx

    def delete_pal(self, pal_id: UUID):
        logger.debug(pal_id)
        for slot in self.slots:
            if are_equal_uuids(slot.pal_id, pal_id):
                self._slots_data.pop(slot.slot_index)
                self.slots.remove(slot)
                break

        # Update slot indexes, ensuring they are in order
        for index, slot in enumerate(self._slots_data):
            self.slots[index].slot_index = index
            PalObjects.set_value(slot["SlotIndex"], value=index)

    def _get_items(self, character_container_save_data: Dict[str, Any]):
        logger.debug("container_id = %s", self.id)
        for character_container in character_container_save_data:
            container_id = PalObjects.get_guid(
                PalObjects.get_nested(character_container, "key", "ID")
            )
            if not are_equal_uuids(self.id, container_id):
                continue
            container_size = PalObjects.get_value(
                character_container["value"]["SlotNum"]
            )
            self.size = container_size
            self._slots_data = PalObjects.get_array_property(
                character_container["value"]["Slots"]
            )
            for slot in self._slots_data:
                slot_index = PalObjects.get_value(slot["SlotIndex"])
                instance_id = PalObjects.get_nested(
                    slot, "RawData", "value", "instance_id"
                )
                instance_id = (
                    instance_id.UUID()
                    if isinstance(instance_id, ArchiveUUID)
                    else instance_id
                )
                self.slots.append(
                    CharacterContainerSlot(slot_index=slot_index, pal_id=instance_id)
                )
            break
        return self
