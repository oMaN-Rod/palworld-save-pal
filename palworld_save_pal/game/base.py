from typing import Any, Dict, List, Optional
from uuid import UUID
import uuid
from pydantic import BaseModel, Field, PrivateAttr, computed_field


from palworld_save_pal.game.character_container import (
    CharacterContainer,
    CharacterContainerType,
)
from palworld_save_pal.game.item_container import ItemContainer, ItemContainerType
from palworld_save_pal.game.pal import Pal, PalDTO
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.dict import safe_remove
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.uuid import are_equal_uuids

logger = create_logger(__name__)


class BaseDTO(BaseModel):
    id: UUID
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
        character_container_save_data: Dict[str, Any] = None,
        map_object_save_data: Dict[str, Any] = None,
        item_container_save_data: Dict[str, Any] = None,
        dynamic_item_save_data: Dict[str, Any] = None,
        **kwargs
    ):
        super().__init__(**kwargs)
        if data:
            self._base_save_data = data
        if character_container_save_data:
            self.pal_container = CharacterContainer(
                id=self.container_id,
                player_uid=PalObjects.EMPTY_UUID,
                type=CharacterContainerType.BASE,
                character_container_save_data=character_container_save_data,
            )
        if map_object_save_data and item_container_save_data and dynamic_item_save_data:
            self._load_storage_containers(
                map_object_save_data, item_container_save_data, dynamic_item_save_data
            )
        if pals:
            self.pals = pals

    @computed_field
    def id(self) -> UUID:
        self._id = PalObjects.as_uuid(self._base_save_data["key"])
        return self._id

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
        new_pal = Pal(new_pal_data)
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
        for id, container in other.storage_containers.items():
            self.storage_containers[id].update_from(container.model_dump())

    def _load_storage_containers(
        self, map_object_save_data, item_container_save_data, dynamic_item_save_data
    ):
        self.storage_containers = {}
        for map_object in map_object_save_data["values"]:
            base_camp_id = PalObjects.as_uuid(
                PalObjects.get_nested(
                    map_object,
                    "Model",
                    "value",
                    "RawData",
                    "value",
                    "base_camp_id_belong_to",
                )
            )
            if not are_equal_uuids(base_camp_id, self.id):
                continue
            self._map_object_save_data.append(map_object)
            module_map = PalObjects.get_nested(
                map_object, "ConcreteModel", "value", "ModuleMap", "value"
            )
            for module in module_map:
                if (
                    module["key"]
                    == "EPalMapObjectConcreteModelModuleType::ItemContainer"
                ):
                    container_id = PalObjects.as_uuid(
                        module["value"]["RawData"]["value"]["target_container_id"]
                    )
                    logger.debug("%s => %s", self.id, container_id)
                    key = PalObjects.get_value(map_object["MapObjectId"])
                    self.storage_containers[container_id] = ItemContainer(
                        id=container_id,
                        key=key,
                        type=ItemContainerType.BASE,
                        item_container_save_data=item_container_save_data,
                        dynamic_item_save_data=dynamic_item_save_data,
                    )
