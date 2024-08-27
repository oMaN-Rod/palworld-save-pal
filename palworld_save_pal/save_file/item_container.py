from typing import Any, Dict, List, Optional
from uuid import UUID
import uuid
from pydantic import BaseModel, Field

from palworld_save_tools.archive import UUID as ArchiveUUID

from palworld_save_pal.save_file.container_slot import ContainerSlot
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
    slots: List[ContainerSlot] = Field(default_factory=list)

    # pylint: disable=E1133
    def get_slot(self, slot_index: int) -> Optional[ContainerSlot]:
        return next(
            (slot for slot in self.slots if slot.slot_index == slot_index), None
        )

    def _get_dynamic_item(
        self, local_id: UUID, dynamic_item_save_data: List[Dict[str, Any]]
    ) -> Optional[DynamicItem]:
        logger.info("Getting dynamic item with local id: %s", local_id)
        item = None
        for entry in dynamic_item_save_data:
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

    def get_items(
        self,
        item_container_save_data: Dict[str, Any],
        dynamic_item_save_data: Dict[str, Any],
    ) -> "ItemContainer":
        logger.info("Getting items for container: %s", self.id)
        self.slots = []
        for entry in item_container_save_data:
            current_container_id = PalObjects.get_guid(
                PalObjects.get_nested(entry, "key", "ID")
            )
            if are_equal_uuids(current_container_id, self.id):
                slots = PalObjects.get_array_property(
                    PalObjects.get_nested(entry, "value", "Slots")
                )
                for slot in slots:
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
                        dynamic_item = self._get_dynamic_item(
                            local_id, dynamic_item_save_data
                        )
                    count = PalObjects.get_value(slot["StackCount"])
                    self.slots.append(
                        ContainerSlot(
                            slot_index=slot_index,
                            static_id=static_id,
                            count=count,
                            dynamic_item=dynamic_item,
                        )
                    )
                break
        return self

    def _set_dynamic_data(
        self,
        static_id: str,
        dynamic_item: DynamicItem,
        dynamic_item_data: Dict[str, Any],
    ) -> Dict[str, Any]:
        logger.info("Setting dynamic data for item: %s", dynamic_item.local_id)
        # Set
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
        self,
        slot: Dict[str, Any],
        container_slot: ContainerSlot,
        dynamic_item_save_data: List[Dict[str, Any]],
    ) -> UUID:
        logger.info("Setting dynamic item for slot: %s", container_slot.slot_index)
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
            for entry in dynamic_item_save_data:
                local_id = PalObjects.get_guid(
                    PalObjects.get_nested(entry, "ID", "value", "LocalIdInCreatedWorld")
                )
                if are_equal_uuids(local_id, slot_local_id):
                    dynamic_item_save_data.remove(entry)
                    break
            return PalObjects.EMPTY_UUID

        if not container_slot.dynamic_item:
            return PalObjects.EMPTY_UUID

        if is_empty_uuid(container_slot.dynamic_item.local_id) and is_empty_uuid(
            slot_local_id
        ):
            container_slot.dynamic_item.local_id = uuid.uuid4()
            new_dynamic_item = PalObjects.DynamicItem(container_slot)
            self._set_dynamic_data(
                container_slot.static_id,
                container_slot.dynamic_item,
                new_dynamic_item,
            )
            dynamic_item_save_data.append(new_dynamic_item)
            return container_slot.dynamic_item.local_id

        if is_empty_uuid(container_slot.dynamic_item.local_id) and not is_empty_uuid(
            slot_local_id
        ):
            container_slot.dynamic_item.local_id = slot_local_id

        # If the dynamic item is not empty, we need to update it
        for entry in dynamic_item_save_data:
            local_id = PalObjects.get_guid(
                PalObjects.get_nested(entry, "ID", "value", "LocalIdInCreatedWorld")
            )
            if are_equal_uuids(local_id, container_slot.dynamic_item.local_id):
                self._set_dynamic_data(
                    container_slot.static_id, container_slot.dynamic_item, entry
                )
                return container_slot.dynamic_item.local_id

    def set_items(
        self,
        item_container_save_data: Dict[str, Any],
        dynamic_item_save_data: List[Dict[str, Any]],
    ):
        logger.info("Setting items for container: %s", self.id)
        for entry in item_container_save_data:
            current_container_id = PalObjects.get_guid(
                PalObjects.get_nested(entry, "key", "ID")
            )
            if not are_equal_uuids(current_container_id, self.id):
                continue
            slots = PalObjects.get_array_property(
                PalObjects.get_nested(entry, "value", "Slots")
            )
            for slot in slots:
                slot_index = PalObjects.get_value(slot["SlotIndex"])
                container_slot = self.get_slot(slot_index)
                local_id = self._set_dynamic_item(
                    slot, container_slot, dynamic_item_save_data
                )
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
            break
