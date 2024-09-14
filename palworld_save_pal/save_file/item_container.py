from typing import Any, Dict, List, Optional
from uuid import UUID
import uuid
from pydantic import BaseModel, Field, PrivateAttr

from palworld_save_tools.archive import UUID as ArchiveUUID

from palworld_save_pal.save_file.item_container_slot import ItemContainerSlot
from palworld_save_pal.save_file.dynamic_item import DynamicItem
from palworld_save_pal.save_file.encoders import custom_uuid_encoder
from palworld_save_pal.save_file.pal_objects import PalObjects
from palworld_save_pal.save_file.utils import (
    are_equal_uuids,
    is_empty_uuid,
    is_valid_uuid,
    safe_remove,
)
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class ItemContainer(BaseModel):
    id: UUID = Field(..., json_encoder=custom_uuid_encoder)
    type: str
    slots: List[ItemContainerSlot] = Field(default_factory=list)

    _container_slots_data: Optional[List[Dict[str, Any]]] = PrivateAttr(
        default_factory=dict
    )
    _dynamic_item_save_data: Optional[List[Dict[str, Any]]] = PrivateAttr(
        default_factory=list
    )

    def __init__(
        self,
        item_container_save_data: Dict[str, Any] = None,
        dynamic_item_save_data: Dict[str, Any] = None,
        **kwargs,
    ):
        super().__init__(**kwargs)
        if item_container_save_data and dynamic_item_save_data:
            self._dynamic_item_save_data = dynamic_item_save_data
            self._get_container_slots(item_container_save_data)
            self._get_items()

    def set_items(self):
        logger.debug("%s (%s)", self.type, self.id)
        for slot in self._container_slots_data:
            slot_index = PalObjects.get_value(slot["SlotIndex"])
            container_slot = self._get_slot(slot_index)
            local_id = self._set_dynamic_item(slot, container_slot)
            PalObjects.set_nested(
                slot,
                "ItemId",
                "value",
                "DynamicId",
                "value",
                "LocalIdInCreatedWorld",
                "value",
                value=str(local_id),
            )
            PalObjects.set_nested(
                slot,
                "ItemId",
                "value",
                "StaticId",
                "value",
                value=container_slot.static_id,
            )
            PalObjects.set_value(slot["StackCount"], value=container_slot.count)

    def update_from(self, other_container: Dict[str, Any]):
        logger.debug("%s (%s) with keys %s", self.type, self.id, other_container.keys())
        for key, value in other_container.items():
            match key:
                case "slots":
                    self.slots = [ItemContainerSlot(**slot) for slot in value]

    def _get_slot(self, slot_index: int) -> Optional[ItemContainerSlot]:
        return next(
            (slot for slot in self.slots if slot.slot_index == slot_index), None
        )

    def _get_container_slots(self, item_container_save_data: Dict[str, Any]):
        logger.debug("%s (%s)", self.type, self.id)
        for entry in item_container_save_data:
            container_id = PalObjects.get_guid(
                PalObjects.get_nested(entry, "key", "ID")
            )
            if are_equal_uuids(container_id, self.id):
                self._container_slots_data = PalObjects.get_array_property(
                    PalObjects.get_nested(entry, "value", "Slots")
                )
                break

    def _get_dynamic_item(self, local_id: UUID) -> Optional[DynamicItem]:
        logger.debug("%s (%s) => %s", self.type, self.id, local_id)
        item = None
        for entry in self._dynamic_item_save_data:
            current_local_id = PalObjects.get_guid(
                PalObjects.get_nested(entry, "ID", "value", "LocalIdInCreatedWorld")
            )
            if are_equal_uuids(current_local_id, local_id):
                item = entry
                break

        if item:
            raw_data = PalObjects.get_value(item["RawData"])
            item_type = PalObjects.get_nested(raw_data, "type")
            durability = PalObjects.get_nested(raw_data, "durability")
            remaining_bullets = PalObjects.get_nested(raw_data, "remaining_bullets")
            item = DynamicItem(
                local_id=(
                    local_id.UUID() if isinstance(local_id, ArchiveUUID) else local_id
                ),
                durability=durability,
                remaining_bullets=remaining_bullets,
                type=item_type,
            )
        return item

    def _get_items(self) -> "ItemContainer":
        logger.debug("%s (%s)", self.type, self.id)
        self.slots = []
        for slot in self._container_slots_data:
            slot_index = PalObjects.get_value(slot["SlotIndex"])
            item_id = PalObjects.get_value(slot["ItemId"])
            static_id = PalObjects.get_value(item_id["StaticId"])
            local_id = PalObjects.get_guid(
                PalObjects.get_nested(
                    item_id, "DynamicId", "value", "LocalIdInCreatedWorld"
                )
            )
            dynamic_item = None
            if not is_empty_uuid(local_id):
                dynamic_item = self._get_dynamic_item(local_id)
                if not dynamic_item:
                    logger.error(
                        "Dynamic item not found for: %s, %s", slot_index, local_id
                    )
                    raise ValueError("Dynamic item not found")

            count = PalObjects.get_value(slot["StackCount"])
            self.slots.append(
                ItemContainerSlot(
                    slot_index=slot_index,
                    static_id=static_id,
                    count=count,
                    dynamic_item=dynamic_item,
                )
            )
        return self

    def _set_dynamic_data(
        self,
        static_id: str,
        dynamic_item: DynamicItem,
        dynamic_item_data: Dict[str, Any],
    ) -> Dict[str, Any]:
        logger.debug("%s (%s) => (%s)", self.type, self.id, dynamic_item)
        PalObjects.set_nested(
            dynamic_item_data,
            "ID",
            "value",
            "LocalIdInCreatedWorld",
            "value",
            value=str(dynamic_item.local_id),
        )
        PalObjects.set_nested(
            dynamic_item_data,
            "RawData",
            "value",
            "id",
            "local_id_in_created_world",
            value=str(dynamic_item.local_id),
        )
        # Set static ID
        PalObjects.set_value(dynamic_item_data["StaticItemId"], value=static_id)
        raw_data = PalObjects.get_value(dynamic_item_data["RawData"])
        PalObjects.set_nested(raw_data, "id", "static_id", value=static_id)
        PalObjects.set_nested(raw_data, "type", value=dynamic_item.type)
        if dynamic_item.type == "armor":
            PalObjects.set_nested(
                raw_data,
                "durability",
                value=dynamic_item.durability,
            )
            safe_remove(raw_data, "remaining_bullets")
            safe_remove(raw_data, "passive_skill_list")

        if dynamic_item.type == "weapon":
            PalObjects.set_nested(
                raw_data,
                "durability",
                value=dynamic_item.durability,
            )
            PalObjects.set_nested(
                raw_data,
                "remaining_bullets",
                value=dynamic_item.remaining_bullets,
            )
            passive_skill_list = PalObjects.get_nested(raw_data, "passive_skill_list")
            if not passive_skill_list:
                raw_data["passive_skill_list"] = []

    def _set_dynamic_item(
        self, slot: Dict[str, Any], container_slot: ItemContainerSlot
    ) -> UUID:
        logger.debug("%s (%s) => %s", self.type, self.id, container_slot)
        slot_local_id = PalObjects.get_guid(
            PalObjects.get_nested(
                slot, "ItemId", "value", "DynamicId", "value", "LocalIdInCreatedWorld"
            )
        )
        # New container slot does not have a dynamic item, we need to check if slot
        # has a dynamic item, if it does we need to delete it
        if (
            not container_slot.dynamic_item
            and not is_empty_uuid(slot_local_id)
            and is_valid_uuid(str(slot_local_id))
        ):
            logger.debug("Deleting dynamic item, found %s", slot_local_id)
            for entry in self._dynamic_item_save_data:
                local_id = PalObjects.get_guid(
                    PalObjects.get_nested(entry, "ID", "value", "LocalIdInCreatedWorld")
                )
                if are_equal_uuids(local_id, slot_local_id):
                    logger.debug("Found dynamic item, deleting %s", slot_local_id)
                    self._dynamic_item_save_data.remove(entry)
                    return PalObjects.EMPTY_UUID
            logger.error("Cannot delete, dynamic item not found %s", slot_local_id)
            return PalObjects.EMPTY_UUID

        if not container_slot.dynamic_item:
            return PalObjects.EMPTY_UUID

        # Create a new dynamic item
        if is_empty_uuid(container_slot.dynamic_item.local_id) and is_empty_uuid(
            slot_local_id
        ):
            logger.debug("Creating new dynamic item")
            container_slot.dynamic_item.local_id = uuid.uuid4()
            new_dynamic_item = PalObjects.DynamicItem(container_slot)
            self._set_dynamic_data(
                container_slot.static_id,
                container_slot.dynamic_item,
                new_dynamic_item,
            )
            self._dynamic_item_save_data.append(new_dynamic_item)
            return container_slot.dynamic_item.local_id

        # ID empty from the front end, but not from the back end
        if is_empty_uuid(container_slot.dynamic_item.local_id) and not is_empty_uuid(
            slot_local_id
        ):
            logger.debug(
                "Container slot has dynamic item, updating ID to %s", slot_local_id
            )
            container_slot.dynamic_item.local_id = slot_local_id

        # If the dynamic item is not empty, we need to update it
        for entry in self._dynamic_item_save_data:
            local_id = PalObjects.get_guid(
                PalObjects.get_nested(entry, "ID", "value", "LocalIdInCreatedWorld")
            )
            if are_equal_uuids(local_id, container_slot.dynamic_item.local_id):
                logger.debug("Updating dynamic item %s", container_slot.dynamic_item)
                self._set_dynamic_data(
                    container_slot.static_id, container_slot.dynamic_item, entry
                )
                return container_slot.dynamic_item.local_id
        logger.error("Dynamic item not found %s", container_slot.dynamic_item)
