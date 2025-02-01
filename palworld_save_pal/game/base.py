from typing import Any, Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel, Field, PrivateAttr, computed_field


from palworld_save_pal.game.pal import Pal
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.uuid import are_equal_uuids, is_empty_uuid
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class Base(BaseModel):
    pals: Optional[Dict[UUID, Pal]] = Field(default_factory=dict)

    _base_save_data: Dict[str, Any] = PrivateAttr(default_factory=dict)
    _raw_data: Dict[str, Any] = PrivateAttr(default_factory=dict)

    def __init__(self, data: Dict[str, Any] = None, pals: List[UUID] = []):
        super().__init__()
        if data:
            self._base_save_data = data
        if pals:
            self.pals = pals

    @computed_field
    def id(self) -> UUID:
        self._id = PalObjects.as_uuid(self._base_save_data["key"])
        return self._id

    def add_pal(self, pal: Pal):
        logger.debug("%s (%s) => %s", self.name, self.id, pal.id)
        self.pals[pal.id] = pal

    def remove_pal(self, pal_id: UUID):
        logger.debug("%s (%s) => %s", self.name, self.id, pal_id)
        del self.pals[pal_id]