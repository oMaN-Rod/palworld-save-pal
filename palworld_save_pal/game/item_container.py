from enum import Enum
from typing import Any, Dict, List, Optional
from uuid import UUID
import uuid
from pydantic import BaseModel, Field, PrivateAttr

from palworld_save_pal.dto.item_container_slot import ItemContainerSlotDTO
from palworld_save_pal.game.item_container_slot import ItemContainerSlot
from palworld_save_pal.game.dynamic_item import DynamicItem
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.uuid import (
    are_equal_uuids,
    is_empty_uuid,
)
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class ItemContainerType(str, Enum):
    COMMON = "CommonContainer"
    ESSENTIAL = "EssentialContainer"
    WEAPON = "WeaponLoadOutContainer"
    ARMOR = "PlayerEquipArmorContainer"
    FOOD = "FoodEquipContainer"
    BASE = "BaseContainer"
    GUILD = "GuildChest"


class ItemContainer(BaseModel):
    id: UUID
    type: ItemContainerType
    slots: List[ItemContainerSlot] = Field(default_factory=list)
    key: Optional[str] = None
    slot_num: int = 0

    _container_slots_data: Optional[List[Dict[str, Any]]] = PrivateAttr(
        default_factory=list
    )
    _dynamic_item_save_data: Optional[List[Dict[str, Any]]] = PrivateAttr(
        default_factory=list
    )
    _dynamic_item_index: Optional[Dict[UUID, Dict[str, Any]]] = PrivateAttr(
        default=None
    )

    def __init__(
        self,
        container_data: Optional[Dict[str, Any]] = None,
        dynamic_item_index: Optional[Dict[UUID, Dict[str, Any]]] = None,
        **kwargs: Any,
    ) -> None:
        super().__init__(**kwargs)
        self._dynamic_item_index = dynamic_item_index

        if container_data is not None and dynamic_item_index is not None:
            self._container_slots_data = PalObjects.get_array_property(
                PalObjects.get_nested(container_data, "value", "Slots")
            )
            self.slot_num = PalObjects.get_value(
                PalObjects.get_nested(container_data, "value", "SlotNum")
            )
            self._get_items()

    def _set_items(self, new_slots: List[ItemContainerSlotDTO]) -> None:
        logger.debug("%s (%s)", self.type.value, self.id)
        for slot_dto in new_slots:
            self._update_or_create_container_slot(slot_dto)

    def update_from(self, other_container: Dict[str, Any]) -> None:
        logger.debug(
            "%s (%s) with keys %s", self.type.value, self.id, other_container.keys()
        )
        for key, value in other_container.items():
            if key == "slots":
                new_slots = [ItemContainerSlotDTO(**slot) for slot in value]
                self._clean_up_inventory(new_slots)
                self._set_items(new_slots)

    def _clean_up_inventory(self, new_slots: List[ItemContainerSlotDTO]) -> None:
        logger.debug("%s (%s)", self.type.value, self.id)
        for slot_dto in new_slots:
            container_slot = next(
                (s for s in self.slots if s.slot_index == slot_dto.slot_index),
                None,
            )
            if (
                not slot_dto.dynamic_item
                and container_slot
                and container_slot.dynamic_item
            ):
                self._dynamic_item_save_data.remove(
                    container_slot.dynamic_item.save_data
                )
                container_slot.dynamic_item = None
            if slot_dto.static_id == "None":
                self._remove_container_slot(slot_dto.slot_index)

    def _get_dynamic_item(self, local_id: UUID) -> Optional[DynamicItem]:
        if self._dynamic_item_index and local_id in self._dynamic_item_index:
            entry = self._dynamic_item_index[local_id]
            return DynamicItem(local_id=local_id, dynamic_item_save_data=entry)
        return None

    def _get_items(self):
        self.slots = []
        for slot_data in self._container_slots_data:
            slot = ItemContainerSlot(container_slot_data=slot_data)
            if slot.local_id and not is_empty_uuid(slot.local_id):
                dynamic_item = self._get_dynamic_item(slot.local_id)
                if not dynamic_item:
                    logger.error(
                        "Dynamic item not found for: slot-%s, %s, %s",
                        slot.slot_index,
                        slot.local_id,
                        slot.static_id,
                    )
                    self._remove_container_slot(slot.slot_index)
                    continue
                slot.dynamic_item = dynamic_item
            self.slots.append(slot)

    def _remove_container_slot(self, slot_index: int) -> None:
        logger.debug("%s (%s) => %s", self.type.value, self.id, slot_index)
        for slot in self._container_slots_data:
            raw_data = PalObjects.get_value(slot["RawData"])
            current_slot_index = PalObjects.get_nested(raw_data, "slot_index")
            if are_equal_uuids(current_slot_index, slot_index):
                logger.debug("Removing slot %s", slot_index)
                self._container_slots_data.remove(slot)
                break
        for slot in self.slots:
            if slot.slot_index == slot_index:
                logger.debug("Removing slot %s", slot_index)
                self.slots.remove(slot)
                break

    def _update_or_create_container_slot(
        self, slot_dto: ItemContainerSlotDTO
    ) -> Optional[Dict[str, Any]]:
        if not slot_dto or slot_dto.static_id == "None":
            return
        logger.debug("%s (%s) => %s", self.type, self.id, slot_dto)
        slot = next(
            (s for s in self.slots if s.slot_index == slot_dto.slot_index), None
        )

        if not slot:
            slot_data = PalObjects.ItemContainerSlot(slot_dto)
            slot = ItemContainerSlot(container_slot_data=slot_data)
            self._container_slots_data.append(slot.slot_data)
            self.slots.append(slot)
        else:
            slot.update_from(slot_dto.model_dump())

        if not slot_dto.dynamic_item:
            return
        if not slot_dto.dynamic_item.local_id or is_empty_uuid(
            slot_dto.dynamic_item.local_id
        ):
            slot_dto.dynamic_item.local_id = (
                uuid.uuid4() if not slot.dynamic_item else slot.dynamic_item.local_id
            )

        if slot.dynamic_item:
            slot.dynamic_item.update_from(slot_dto.dynamic_item.model_dump())
        else:
            new_item = PalObjects.DynamicItem(slot_dto)
            slot.dynamic_item = DynamicItem(
                local_id=slot_dto.dynamic_item.local_id, dynamic_item_save_data=new_item
            )
            slot.dynamic_item.update_from(slot_dto.dynamic_item.model_dump())
            self._dynamic_item_save_data.append(slot.dynamic_item.save_data)
        slot.local_id = slot_dto.dynamic_item.local_id
