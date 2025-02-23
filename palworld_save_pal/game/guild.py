import copy
from typing import Any, Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel, Field, PrivateAttr, computed_field

from palworld_save_pal.game.base import Base, BaseDTO
from palworld_save_pal.game.item_container import ItemContainer, ItemContainerType
from palworld_save_pal.game.pal import PalDTO
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.uuid import are_equal_uuids, is_empty_uuid
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class GuildDTO(BaseModel):
    name: Optional[str] = None
    base: Optional[BaseDTO] = None
    guild_chest: Optional[ItemContainer] = None


class Guild(BaseModel):
    _id: UUID
    _admin_player_uid: UUID
    _name: str
    _players = List[UUID]

    _group_save_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _guild_extra_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _raw_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _character_handle_ids: Optional[List[Dict[str, Any]]] = PrivateAttr(
        default_factory=list
    )

    bases: Dict[UUID, Base] = Field(default_factory=dict)
    guild_chest: Optional[ItemContainer] = Field(default=None)

    def __init__(
        self,
        group_save_data: Dict[str, Any] = None,
        guild_extra_data: Dict[str, Any] = None,
        item_container_save_data: Dict[str, Any] = None,
        dynamic_item_save_data: Dict[str, Any] = None,
    ):
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
        if guild_extra_data and item_container_save_data and dynamic_item_save_data:
            self._guild_extra_data = guild_extra_data
            self._load_guild_chest(item_container_save_data, dynamic_item_save_data)

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
        self._name = PalObjects.get_nested(self._raw_data, "guild_name")
        return self._name

    @name.setter
    def name(self, value: str):
        self._name = value
        PalObjects.set_nested(self._raw_data, "guild_name", value=self._name)

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

    @computed_field
    def container_id(self) -> Optional[UUID]:
        if self._guild_extra_data is None:
            return
        return PalObjects.as_uuid(
            PalObjects.get_nested(
                self._guild_extra_data,
                "value",
                "GuildItemStorage",
                "value",
                "RawData",
                "value",
                "container_id",
            )
        )

    @property
    def save_data(self) -> Dict[str, Any]:
        result = copy.deepcopy(self._group_save_data)
        if self._guild_extra_data:
            result["guild_extra_data"] = self._guild_extra_data
        return result

    def add_pal(self, pal_id: UUID):
        logger.debug("%s (%s) => %s", self.name, self.id, pal_id)
        new_pal = PalObjects.individual_character_handle_ids(pal_id)
        self._character_handle_ids.append(new_pal)

    def add_base_pal(
        self, character_id: str, nickname: str, base_id: UUID, storage_slot: int = None
    ):
        logger.debug("%s (%s) => %s", self.name, self.id, character_id)
        new_pal = self.bases[base_id].add_pal(character_id, nickname, storage_slot)
        if new_pal is None:
            return
        self.add_pal(new_pal.instance_id)
        return new_pal

    def clone_base_pal(self, base_id: UUID, pal: PalDTO):
        logger.debug(
            "%s (%s) => %s (%s)", self.name, self.id, pal.character_id, pal.instance_id
        )
        new_pal = self.bases[base_id].clone_pal(pal)
        if new_pal is None:
            return
        self.add_pal(new_pal.instance_id)
        return new_pal

    def delete_base_pal(self, base_id: UUID, pal_id: UUID):
        logger.debug("%s (%s) => %s", self.name, self.id, pal_id)
        self.bases[base_id].delete_pal(pal_id)
        self.delete_pal(pal_id)

    def delete_pal(self, pal_id: UUID):
        logger.debug("%s (%s) => %s", self.name, self.id, pal_id)
        for entry in self._character_handle_ids:
            instance_id = PalObjects.as_uuid(entry["instance_id"])
            if are_equal_uuids(instance_id, pal_id):
                self._character_handle_ids.remove(entry)
                logger.debug("%s (%s) => Removed %s", self.name, self.id, pal_id)

    def add_base(self, base: Base):
        self.bases[base.id] = base
        logger.debug("Added base %s to guild %s", base.id, self.id)

    def update_from(self, guildDTO: GuildDTO):
        if guildDTO.name:
            self.name = guildDTO.name
        if guildDTO.base:
            base = next(
                b
                for b in self.bases.values()
                if are_equal_uuids(b.id, guildDTO.base.id)
            )
            base.update_from(guildDTO.base)
        if guildDTO.guild_chest:
            if self.guild_chest is None:
                return
            self.guild_chest.update_from(guildDTO.guild_chest.model_dump())

    def _load_guild_chest(self, item_container_save_data, dynamic_item_save_data):
        if self.container_id is None:
            return
        self.guild_chest = ItemContainer(
            id=self.container_id,
            key="GuildChest",
            type=ItemContainerType.GUILD,
            item_container_save_data=item_container_save_data,
            dynamic_item_save_data=dynamic_item_save_data,
        )
        logger.debug("Loaded guild chest %s", self.container_id)

    def nuke(self):
        """
        Deletes the entire guild and all associated data.

        Note:
            Use this function with caution as it will result in the loss of all guild data.
        """
        logger.info("Nuking guild %s (%s)", self.name, self.id)
        for base in self.bases.values():
            base.nuke()
        
        self.bases.clear()
        
        if self.guild_chest:
            self.guild_chest.nuke()

        self._group_save_data = None
