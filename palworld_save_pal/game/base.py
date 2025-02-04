from typing import Any, Dict, List, Optional
from uuid import UUID
import uuid
from pydantic import BaseModel, Field, PrivateAttr, computed_field


from palworld_save_pal.game.character_container import (
    CharacterContainer,
    CharacterContainerType,
)
from palworld_save_pal.game.pal import Pal, PalDTO
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.uuid import are_equal_uuids, is_empty_uuid
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class Base(BaseModel):
    pals: Optional[Dict[UUID, Pal]] = Field(default_factory=dict)
    container_id: Optional[UUID] = None
    slot_count: Optional[int] = None

    _pal_container: CharacterContainer
    _base_save_data: Dict[str, Any] = PrivateAttr(default_factory=dict)
    _raw_data: Dict[str, Any] = PrivateAttr(default_factory=dict)

    def __init__(
        self,
        data: Dict[str, Any] = None,
        pals: List[UUID] = [],
        character_container_save_data: Dict[str, Any] = None,
        **kwargs
    ):
        super().__init__(**kwargs)
        if data and character_container_save_data:
            self._base_save_data = data
            self._pal_container = CharacterContainer(
                id=self.container_id,
                player_uid=PalObjects.EMPTY_UUID,
                type=CharacterContainerType.BASE,
                character_container_save_data=character_container_save_data,
            )
        if pals:
            self.pals = pals

    @computed_field
    def id(self) -> UUID:
        self._id = PalObjects.as_uuid(self._base_save_data["key"])
        return self._id

    def add_pal(
        self, character_id: str, nickname: str, storage_slot: Optional[UUID] = None
    ) -> Pal | None:
        logger.debug("%s => %s (%s) #%s", self.id, character_id, nickname, storage_slot)
        new_pal_id = uuid.uuid4()
        slot_idx = self._pal_container.add_pal(new_pal_id, storage_slot)
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
        return new_pal, new_pal_data

    def clone_pal(self, pal: PalDTO) -> Pal | None:
        new_pal_id = uuid.uuid4()
        slot_idx = self._pal_container.add_pal(new_pal_id)
        if slot_idx is None:
            return
        existing_pal = self.pals[pal.instance_id]
        nickname = pal.nickname if pal.nickname else pal.character_id
        new_pal = existing_pal.clone(new_pal_id, self.container_id, slot_idx, nickname)
        self.pals[new_pal_id] = new_pal
        return new_pal

    def delete_pal(self, pal_id: UUID):
        logger.debug("%s => %s", self.id, pal_id)
        del self.pals[pal_id]
        self._pal_container.remove_pal(pal_id)
