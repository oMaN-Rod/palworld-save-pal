from typing import Any, Dict, Optional
from uuid import UUID

from pydantic import BaseModel, computed_field

from palworld_save_pal.game.dynamic_item import DynamicItem
from palworld_save_pal.game.pal_objects import PalObjects


class ItemContainerSlot(BaseModel):
    dynamic_item: Optional[DynamicItem] = None

    _raw_data: Optional[Dict[str, Any]] = None
    _container_slot__data: Optional[Dict[str, Any]] = None

    def __init__(self, container_slot_data: Dict[str, Any], **kwargs: Any) -> None:
        super().__init__(**kwargs)
        if container_slot_data:
            self._container_slot__data = container_slot_data
            self._raw_data = PalObjects.get_value(container_slot_data["RawData"])

    @computed_field
    def slot_index(self) -> int:
        return PalObjects.get_nested(self._raw_data, "slot_index")

    @slot_index.setter
    def slot_index(self, value: int) -> None:
        PalObjects.set_nested(self._raw_data, "slot_index", value=value)

    @computed_field
    def count(self) -> int:
        return PalObjects.get_nested(self._raw_data, "count")

    @count.setter
    def count(self, value: int) -> None:
        PalObjects.set_nested(self._raw_data, "count", value=value)

    @computed_field
    def static_id(self) -> Optional[str]:
        return PalObjects.get_nested(self._raw_data, "item", "static_id")

    @static_id.setter
    def static_id(self, value: Optional[str]) -> None:
        PalObjects.set_nested(self._raw_data, "item", "static_id", value=value)

    @computed_field
    def dynamic_static_id(self) -> Optional[str]:
        return PalObjects.get_nested(self._raw_data, "item", "dynamic_id", "static_id")

    @dynamic_static_id.setter
    def dynamic_static_id(self, value: Optional[str]) -> None:
        PalObjects.set_nested(
            self._raw_data, "item", "dynamic_id", "static_id", value=value
        )

    @computed_field
    def local_id(self) -> Optional[UUID]:
        return PalObjects.as_uuid(
            PalObjects.get_nested(
                self._raw_data,
                "item",
                "dynamic_id",
                "local_id_in_created_world",
            )
        )

    @local_id.setter
    def local_id(self, value: Optional[str]) -> None:
        PalObjects.set_nested(
            self._raw_data,
            "item",
            "dynamic_id",
            "local_id_in_created_world",
            value=value,
        )

    @property
    def slot_data(self) -> Dict[str, Any]:
        return self._container_slot__data

    def update_from(self, data: Dict[str, Any]) -> None:
        for key, value in data.items():
            if key == "dynamic_item":
                continue
            if hasattr(self, key):
                setattr(self, key, value)
