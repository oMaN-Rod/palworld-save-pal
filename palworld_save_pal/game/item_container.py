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

    def __init__(
        self,
        item_container_save_data: Optional[Dict[str, Any]] = None,
        dynamic_item_save_data: Optional[Dict[str, Any]] = None,
        **kwargs: Any,
    ) -> None:
        super().__init__(**kwargs)
        if item_container_save_data and dynamic_item_save_data:
            self._dynamic_item_save_data = dynamic_item_save_data
            self._get_container_slots(item_container_save_data)
            self._get_items()

    def _set_items(self, new_slots: List[ItemContainerSlotDTO]) -> None:
        logger.debug("%s (%s)", self.type.value, self.id)
        for slot in new_slots:
            slot_data = self._update_or_create_container_slot(slot)
            self._update_or_create_dynamic_item(slot, slot_data)

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

    def _get_container_slots(self, item_container_save_data: Dict[str, Any]) -> None:
        logger.debug("%s (%s)", self.type.value, self.id)
        for entry in item_container_save_data:
            container_id = PalObjects.get_guid(
                PalObjects.get_nested(entry, "key", "ID")
            )
            if are_equal_uuids(container_id, self.id):
                self._container_slots_data = PalObjects.get_array_property(
                    PalObjects.get_nested(entry, "value", "Slots")
                )
                self.slot_num = PalObjects.get_value(
                    PalObjects.get_nested(entry, "value", "SlotNum")
                )
                break

    def _get_dynamic_item(self, local_id: UUID) -> Optional[DynamicItem]:
        logger.debug("%s (%s) => %s", self.type.value, self.id, local_id)
        for entry in self._dynamic_item_save_data:
            local_id_in_created_world = PalObjects.as_uuid(
                PalObjects.get_nested(
                    entry, "RawData", "value", "id", "local_id_in_created_world"
                )
            )
            if are_equal_uuids(local_id_in_created_world, local_id):
                return DynamicItem(local_id=local_id, dynamic_item_save_data=entry)
        return

    def _get_items(self):
        logger.debug("%s (%s)", self.type.value, self.id)
        self.slots = []
        for slot in self._container_slots_data:
            raw_data = PalObjects.get_value(slot["RawData"])
            slot_index = PalObjects.get_nested(raw_data, "slot_index")
            count = PalObjects.get_nested(raw_data, "count")
            static_id = PalObjects.get_nested(raw_data, "item", "static_id")
            local_id = PalObjects.as_uuid(
                PalObjects.get_nested(
                    raw_data, "item", "dynamic_id", "local_id_in_created_world"
                )
            )
            dynamic_item = None
            if local_id and not is_empty_uuid(local_id):
                dynamic_item = self._get_dynamic_item(local_id)
                if not dynamic_item:
                    logger.error(
                        "Dynamic item not found for: slot-%s, %s, %s",
                        slot_index,
                        local_id,
                        static_id,
                    )
                    self._remove_container_slot(slot_index)
                    continue

            self.slots.append(
                ItemContainerSlot(
                    slot_index=slot_index,
                    static_id=static_id,
                    count=count,
                    dynamic_item=dynamic_item,
                )
            )

    def _remove_container_slot(self, slot_index: int) -> None:
        logger.debug("%s (%s) => %s", self.type.value, self.id, slot_index)
        for slot in self._container_slots_data:
            raw_data = PalObjects.get_value(slot["RawData"])
            current_slot_index = PalObjects.get_nested(raw_data, "slot_index")
            if are_equal_uuids(current_slot_index, slot_index):
                logger.debug("Removing slot %s", slot_index)
                self._container_slots_data.remove(slot)
                break

    def _update_or_create_container_slot(
        self, slot_dto: ItemContainerSlotDTO
    ) -> Dict[str, Any]:
        logger.debug("%s (%s) => %s", self.type, self.id, slot_dto)
        slot_data = next(
            (
                s
                for s in self._container_slots_data
                if PalObjects.get_nested(
                    PalObjects.get_value(s["RawData"]), "slot_index"
                )
                == slot_dto.slot_index
            ),
            None,
        )

        if not slot_data:
            slot_data = PalObjects.ItemContainerSlot(slot_dto)
            self._container_slots_data.append(slot_data)
            self.slots.append(
                ItemContainerSlot(
                    slot_index=slot_dto.slot_index,
                    static_id=slot_dto.static_id,
                    count=slot_dto.count,
                    dynamic_item=None,
                )
            )
        else:
            self._update_container_slot(slot_dto, slot_data)
        return slot_data

    def _update_container_slot(
        self, slot_dto: ItemContainerSlotDTO, slot_data: Dict[str, Any]
    ) -> None:
        logger.debug("%s (%s) => %s", self.type, self.id, slot_dto)
        raw_data = PalObjects.get_value(slot_data["RawData"])
        PalObjects.set_nested(raw_data, "slot_index", value=slot_dto.slot_index)
        PalObjects.set_nested(raw_data, "count", value=slot_dto.count)
        PalObjects.set_nested(raw_data, "item", "static_id", value=slot_dto.static_id)

    def _update_or_create_dynamic_item(
        self, slot_dto: ItemContainerSlotDTO, slot_data: Dict[str, Any]
    ) -> None:
        if not slot_dto.dynamic_item:
            return
        logger.debug("%s (%s) => %s", self.type, self.id, slot_dto)
        slot = next(
            (s for s in self.slots if s.slot_index == slot_dto.slot_index), None
        )

        if slot.dynamic_item:
            slot.dynamic_item.update_from(slot_dto.dynamic_item.model_dump())
        else:
            slot_dto.dynamic_item.local_id = uuid.uuid4()
            new_item = PalObjects.DynamicItem(slot_dto)
            slot.dynamic_item = DynamicItem(
                local_id=slot_dto.dynamic_item.local_id, dynamic_item_save_data=new_item
            )
            slot.dynamic_item.update_from(slot_dto.dynamic_item.model_dump())
            self._dynamic_item_save_data.append(slot.dynamic_item.save_data)

        raw_data = PalObjects.get_value(slot_data["RawData"])
        PalObjects.set_nested(
            raw_data,
            "item",
            "dynamic_id",
            "local_id_in_created_world",
            value=(
                slot_dto.dynamic_item.local_id
                if slot_dto.dynamic_item
                else PalObjects.EMPTY_UUID
            ),
        )
