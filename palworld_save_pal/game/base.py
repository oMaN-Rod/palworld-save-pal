from typing import Any, Dict, List, Optional
from uuid import UUID
import uuid
from pydantic import BaseModel, Field, PrivateAttr, computed_field


from palworld_save_pal.game.character_container import (
    CharacterContainer,
    CharacterContainerType,
)
from palworld_save_pal.game.item_container import ItemContainer, ItemContainerType
from palworld_save_pal.game.map import WorldMapPoint
from palworld_save_pal.game.pal import Pal
from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.dict import safe_remove
from palworld_save_pal.utils.indexed_collection import IndexedCollection
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class BaseDTO(BaseModel):
    id: UUID
    name: Optional[str] = None
    storage_containers: Dict[UUID, ItemContainer]


class Base(BaseModel):
    pals: Optional[Dict[UUID, Pal]] = Field(default_factory=dict)
    container_id: Optional[UUID] = None
    slot_count: Optional[int] = None
    storage_containers: Optional[Dict[UUID, ItemContainer]] = Field(default=None)
    pal_container: Optional[CharacterContainer] = None

    _base_save_data: Dict[str, Any] = PrivateAttr(default_factory=dict)
    _map_object_save_data: List[Dict[str, Any]] = PrivateAttr(default_factory=list)

    def __init__(
        self,
        data: Dict[str, Any] = None,
        pals: List[UUID] = [],
        character_container_index: Dict[UUID, Dict[str, Any]] = None,
        base_map_objects: List[Dict[str, Any]] = None,
        item_container_index: Dict[UUID, Dict[str, Any]] = None,
        dynamic_items: Optional[IndexedCollection[UUID, Dict[str, Any]]] = None,
        **kwargs,
    ):
        super().__init__(**kwargs)
        if data:
            self._base_save_data = data
        if character_container_index is not None and self.container_id:
            container_data = character_container_index.get(self.container_id)
            if container_data:
                self.pal_container = CharacterContainer(
                    id=self.container_id,
                    player_uid=PalObjects.EMPTY_UUID,
                    type=CharacterContainerType.BASE,
                    container_data=container_data,
                )
        if base_map_objects is not None and item_container_index and dynamic_items:
            self._load_storage_containers(
                base_map_objects,
                item_container_index,
                dynamic_items,
            )
        if pals:
            self.pals = pals

    @computed_field
    def id(self) -> UUID:
        return PalObjects.as_uuid(self._base_save_data["key"])

    @computed_field
    def name(self) -> str:
        return PalObjects.get_nested(
            self._base_save_data, "value", "RawData", "value", "name"
        )

    @name.setter
    def name(self, value: str):
        if not self._base_save_data:
            return
        logger.debug("%s => %s", self.id, value)
        PalObjects.set_nested(
            self._base_save_data, "value", "RawData", "value", "name", value=value
        )

    @computed_field
    def location(self) -> Optional[WorldMapPoint]:
        last_location = PalObjects.get_nested(
            self._base_save_data,
            "value",
            "RawData",
            "value",
            "transform",
            "translation",
        )
        return (
            WorldMapPoint(
                x=last_location["x"],
                y=last_location["y"],
                z=last_location["z"],
            )
            if last_location
            else None
        )

    @property
    def save_data(self) -> Dict[str, Any]:
        return {
            "base_save_data": {**self._base_save_data},
            "map_object_save_data": self._map_object_save_data,
        }

    def add_pal(
        self, character_id: str, nickname: str, storage_slot: Optional[UUID] = None
    ) -> Pal | None:
        logger.debug("%s => %s (%s) #%s", self.id, character_id, nickname, storage_slot)
        new_pal_id = uuid.uuid4()
        slot_idx = self.pal_container.add_pal(new_pal_id, storage_slot)
        if slot_idx is None:
            return

        group_id = PalObjects.as_uuid(
            PalObjects.get_nested(
                self._base_save_data,
                "value",
                "RawData",
                "value",
                "group_id_belong_to",
            )
        )
        new_pal_data = PalObjects.PalSaveParameter(
            character_id=character_id,
            instance_id=new_pal_id,
            owner_uid=PalObjects.EMPTY_UUID,
            container_id=self.container_id,
            slot_idx=slot_idx,
            group_id=group_id,
            nickname=nickname,
        )
        new_pal = Pal(new_pal_data, new_pal=True)
        self.pals[new_pal.instance_id] = new_pal
        safe_remove(new_pal.character_save, "OwnerPlayerUId")
        return new_pal

    def clone_pal(self, pal: PalDTO) -> Pal | None:
        new_pal_id = uuid.uuid4()
        slot_idx = self.pal_container.add_pal(new_pal_id)
        if slot_idx is None:
            return
        existing_pal = self.pals[pal.instance_id]
        nickname = pal.nickname if pal.nickname else pal.character_id
        new_pal = existing_pal.clone(new_pal_id, self.container_id, slot_idx, nickname)
        safe_remove(new_pal.character_save, "OwnerPlayerUId")
        new_pal.character_save["key"]["PlayerUId"]["value"] = PalObjects.EMPTY_UUID
        self.pals[new_pal_id] = new_pal
        return new_pal

    def delete_pal(self, pal_id: UUID):
        logger.debug("%s => %s", self.id, pal_id)
        del self.pals[pal_id]
        self.pal_container.remove_pal(pal_id)

    def update_from(self, other: BaseDTO):
        logger.debug("%s <= %s", self.id, other.id)
        for id, container in other.storage_containers.items():
            self.storage_containers[id].update_from(container.model_dump())
        if other.name:
            self.name = other.name
        else:
            logger.warning("Base name is empty")

    def _load_storage_containers(
        self,
        base_map_objects: List[Dict[str, Any]],
        item_container_index: Dict[UUID, Dict[str, Any]],
        dynamic_items: IndexedCollection[UUID, Dict[str, Any]],
    ):
        self.storage_containers = {}
        for map_object in base_map_objects:
            self._map_object_save_data.append(map_object)
            try:
                module_map = map_object["ConcreteModel"]["value"]["ModuleMap"]["value"]
            except (KeyError, TypeError):
                continue
            for module in module_map:
                if (
                    module["key"]
                    == "EPalMapObjectConcreteModelModuleType::ItemContainer"
                ):
                    try:
                        container_id = PalObjects.as_uuid(
                            module["value"]["RawData"]["value"]["target_container_id"]
                        )
                    except (KeyError, TypeError):
                        continue
                    if container_id and container_id in item_container_index:
                        key = PalObjects.get_value(map_object["MapObjectId"])
                        self.storage_containers[container_id] = ItemContainer(
                            id=container_id,
                            key=key,
                            type=ItemContainerType.BASE,
                            container_data=item_container_index[container_id],
                            dynamic_items=dynamic_items,
                        )
