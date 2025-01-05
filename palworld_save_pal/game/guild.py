from typing import Any, Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel, Field, PrivateAttr, computed_field


from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.uuid import are_equal_uuids, is_empty_uuid
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class Guild(BaseModel):
    _id: UUID
    _admin_player_uid: UUID
    _name: str
    _players = List[UUID]

    _group_save_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _raw_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _character_handle_ids: Optional[List[Dict[str, Any]]] = PrivateAttr(
        default_factory=list
    )

    def __init__(self, group_save_data: Dict[str, Any] = None):
        super().__init__()
        if group_save_data:
            self._group_save_data = group_save_data
            self._raw_data = PalObjects.get_value(
                self._group_save_data["value"]["RawData"]
            )
            self._character_handle_ids = PalObjects.get_nested(
                self._raw_data,
                "individual_character_handle_ids",
            )

    @computed_field
    def id(self) -> UUID:
        self._id = PalObjects.as_uuid(self._group_save_data["key"])
        return self._id

    @computed_field
    def admin_player_uid(self) -> UUID:
        self._admin_player_uid = PalObjects.as_uuid(
            PalObjects.get_nested(self._raw_data, "admin_player_uid")
        )
        return self._admin_player_uid

    @computed_field
    def name(self) -> str:
        self._name = PalObjects.get_nested(self._raw_data, "name")
        return self._name

    @computed_field
    def players(self) -> List[UUID]:
        players = PalObjects.get_nested(self._raw_data, "players")
        if players:
            self._players = []
            for player in players:
                player_uid = PalObjects.as_uuid(
                    PalObjects.get_nested(player, "player_uid")
                )
                self._players.append(player_uid)
        return self._players

    def add_pal(self, pal_id: UUID):
        logger.debug("%s (%s) => %s", self.name, self.id, pal_id)
        new_pal = PalObjects.individual_character_handle_ids(pal_id)
        self._character_handle_ids.append(new_pal)

    def remove_pal(self, pal_id: UUID):
        logger.debug("%s (%s) => %s", self.name, self.id, pal_id)
        for entry in self._character_handle_ids:
            instance_id = PalObjects.as_uuid(entry["instance_id"])
            if are_equal_uuids(instance_id, pal_id):
                self._character_handle_ids.remove(entry)
                logger.debug("%s (%s) => Removed %s", self.name, self.id, pal_id)
                return True
        return False
