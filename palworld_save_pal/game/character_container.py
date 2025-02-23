from enum import Enum
from typing import Any, Dict, List, Optional, Union
from uuid import UUID
from pydantic import BaseModel, Field, PrivateAttr

from palworld_save_tools.archive import UUID as ArchiveUUID

from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.uuid import are_equal_uuids
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class CharacterContainerType(str, Enum):
    PAL_BOX = "PalBox"
    PARTY = "Party"
    BASE = "Base"


class CharacterContainerSlot(BaseModel):
    slot_index: int
    pal_id: Optional[UUID] = None


class CharacterContainer(BaseModel):
    id: UUID
    player_uid: UUID
    type: CharacterContainerType
    size: Optional[int] = 0
    slots: Optional[List[CharacterContainerSlot]] = Field(default_factory=list)

    _slots_data: Optional[List[Dict[str, Any]]] = PrivateAttr(default_factory=list)

    def __init__(self, character_container_save_data: Dict[str, Any] = None, **kwargs):
        super().__init__(**kwargs)
        if character_container_save_data is not None:
            self._get_characters(character_container_save_data)

    def available_slots(self) -> bool:
        return len(self.slots) < self.size

    def find_first_available_slot(self) -> int:
        used_slots = set(slot.slot_index for slot in self.slots)
        for i in range(self.size):
            if i not in used_slots:
                return i

    def find_last_available_slot(self) -> int:
        used_slots = set(slot.slot_index for slot in self.slots)
        for i in range(self.size - 1, -1, -1):
            if i not in used_slots:
                return i

    def add_pal(
        self, pal_id: UUID, storage_slot: Union[int | None] = None
    ) -> Optional[int]:
        if not self.available_slots():
            logger.warning(
                "%s (%s) is full, size is %s", self.type.value, self.id, len(self.slots)
            )
            return
        slot_idx = (
            storage_slot
            if storage_slot is not None
            else self.find_first_available_slot()
        )
        logger.debug(
            "%s (%s) => pal_id = %s, slot_idx = %s, container_id = %s",
            self.type.value,
            self.id,
            pal_id,
            slot_idx,
            self.id,
        )
        new_container_slot_data = PalObjects.ContainerSlotData(
            slot_idx=slot_idx, instance_id=pal_id, player_uid=self.player_uid
        )
        self._slots_data.append(new_container_slot_data)
        if not self.slots:
            self.slots = []
        self.slots.append(CharacterContainerSlot(slot_index=slot_idx, pal_id=pal_id))
        return slot_idx

    def remove_pal(self, pal_id: UUID):
        logger.debug("%s (%s) => %s", self.type.value, self.id, pal_id)
        for slot in self.slots:
            if are_equal_uuids(slot.pal_id, pal_id):
                self._delete_slot_data(slot.slot_index)
                self.slots.remove(slot)
                logger.debug("%s (%s) => Removed %s", self.type.value, self.id, pal_id)
                break

    def _delete_slot_data(self, slot_index: int):
        logger.debug("%s (%s) => index: %s", self.type.value, self.id, slot_index)
        for slot in self._slots_data:
            current_slot_index = PalObjects.get_value(slot["SlotIndex"])
            if current_slot_index == slot_index:
                self._slots_data.remove(slot)
                logger.debug(
                    "%s (%s) => Removed index %s",
                    self.type.value,
                    self.id,
                    slot_index,
                )
                break

    def _order_slots(self):
        for index, slot in enumerate(self._slots_data):
            self.slots[index].slot_index = index
            PalObjects.set_value(slot["SlotIndex"], value=index)

    def _get_characters(self, character_container_save_data: Dict[str, Any]):
        logger.debug("%s (%s)", self.type.value, self.id)
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
        logger.debug(
            "%s (%s) => slots: %s, slots data: %s",
            self.type.value,
            self.id,
            self.size,
            len(self.slots),
        )
        return self

    def nuke(self):
        logger.debug("Nuking %s (%s)", self.type.value, self.id)
        self.slots = []
        self._slots_data = []
        self.size = 0
