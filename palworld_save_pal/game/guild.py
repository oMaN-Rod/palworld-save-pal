from typing import Any, Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel, Field, PrivateAttr


from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.uuid import are_equal_uuids, is_empty_uuid
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class Guild(BaseModel):
    id: UUID
    admin_player_uid: Optional[UUID] = None
    name: Optional[str] = None
    players: Optional[list[UUID]] = Field(default_factory=list)

    _group_save_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _individual_character_handle_ids: Optional[List[Dict[str, Any]]] = PrivateAttr(
        default_factory=list
    )

    def __init__(
        self,
        group_save_data: Dict[str, Any] = None,
        **kwargs,
    ):
        super().__init__(**kwargs)
        if group_save_data:
            self._group_save_data = group_save_data
            self.load_guild_data()

    def add_pal(self, pal_id: UUID):
        logger.debug("%s (%s) => %s", self.name, self.id, pal_id)
        new_pal = PalObjects.individual_character_handle_ids(pal_id)
        self._individual_character_handle_ids.append(new_pal)

    def remove_pal(self, pal_id: UUID):
        logger.debug("%s (%s) => %s", self.name, self.id, pal_id)
        for entry in self._individual_character_handle_ids:
            instance_id = PalObjects.as_uuid(entry["instance_id"])
            if are_equal_uuids(instance_id, pal_id):
                self._individual_character_handle_ids.remove(entry)
                logger.debug("%s (%s) => Removed %s", self.name, self.id, pal_id)
                return True
        return False

    def load_guild_data(self):
        self._load_individual_character_handle_ids()
        self._load_players()

    def _load_guild_name(self):
        self.name = PalObjects.get_nested(
            self._group_save_data, "value", "RawData", "value", "name"
        )

    def _load_individual_character_handle_ids(self):
        self._individual_character_handle_ids = PalObjects.get_nested(
            self._group_save_data,
            "value",
            "RawData",
            "value",
            "individual_character_handle_ids",
        )

    def _load_players(self):
        for entry in self._individual_character_handle_ids:
            if "guid" in entry and not is_empty_uuid(entry["guid"]):
                self.players.append(PalObjects.as_uuid(entry["guid"]))
