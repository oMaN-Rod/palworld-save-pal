import copy
from typing import Any, Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel, Field, PrivateAttr, computed_field

from palworld_save_pal.dto.guild import GuildDTO
from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.game.base import Base
from palworld_save_pal.game.guild_lab_research_info import GuildLabResearchInfo
from palworld_save_pal.game.item_container import ItemContainer, ItemContainerType
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.uuid import are_equal_uuids
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class Guild(BaseModel):
    _group_save_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _guild_extra_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _raw_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _lab_raw_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _character_handle_ids: Optional[List[Dict[str, Any]]] = PrivateAttr(
        default_factory=list
    )

    bases: Dict[UUID, Base] = Field(default_factory=dict)
    guild_chest: Optional[ItemContainer] = Field(default=None)
    lab_research: List[GuildLabResearchInfo] = Field(default_factory=list)

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
        if guild_extra_data:
            self._guild_extra_data = guild_extra_data
            if item_container_save_data and dynamic_item_save_data:
                self._load_guild_chest(item_container_save_data, dynamic_item_save_data)
            self._load_lab_research()

    @computed_field
    def id(self) -> UUID:
        return PalObjects.as_uuid(self._group_save_data["key"])

    @computed_field
    def admin_player_uid(self) -> UUID:
        return self.players[0] if self.players else None

    @computed_field
    def name(self) -> str:
        return PalObjects.get_nested(self._raw_data, "guild_name")

    @name.setter
    def name(self, value: str):
        PalObjects.set_nested(self._raw_data, "guild_name", value=value)

    @computed_field
    def players(self) -> List[UUID]:
        players_data = PalObjects.get_nested(self._raw_data, "players")
        if players_data:
            players = []
            for player in players_data:
                player_uid = PalObjects.as_uuid(
                    PalObjects.get_nested(player, "player_uid")
                )
                players.append(player_uid)
        return players

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

    @computed_field
    def lab_research_data(self) -> List[GuildLabResearchInfo]:
        return self.lab_research

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
        self.delete_character_handle(pal_id)

    def delete_character_handle(self, target_id: UUID):
        logger.debug("%s (%s) => %s", self.name, self.id, target_id)
        for entry in self._character_handle_ids:
            instance_id = PalObjects.as_uuid(entry["instance_id"])
            guid = PalObjects.as_uuid(entry["guid"])
            if are_equal_uuids(instance_id, target_id) or are_equal_uuids(
                guid, target_id
            ):
                self._character_handle_ids.remove(entry)
                logger.debug("%s (%s) => Removed %s", self.name, self.id, target_id)

    def delete_player(self, player_uid: UUID):
        logger.debug("%s (%s) => %s", self.name, self.id, player_uid)
        for entry in self.players:
            if are_equal_uuids(entry, player_uid):
                self.players.remove(entry)
                self.delete_character_handle(player_uid)
                self._raw_data["players"][:] = [
                    player
                    for player in self._raw_data["players"]
                    if player_uid != player["player_uid"]
                ]
                logger.debug("%s (%s) => Removed %s", self.name, self.id, player_uid)

    def add_base(self, base: Base):
        self.bases[base.id] = base
        logger.debug("Added base %s to guild %s", base.id, self.id)

    def _load_lab_research(self):
        if not self._guild_extra_data:
            logger.warning("No guild extra data found for guild %s", self.id)
            return

        lab_data = PalObjects.get_nested(
            self._guild_extra_data, "value", "Lab", "value", "RawData", "value"
        )
        if not lab_data:
            logger.warning(
                "No Lab data found in guild extra data for guild %s", self.id
            )
            return

        self._lab_raw_data = lab_data
        research_info_list = PalObjects.get_nested(self._lab_raw_data, "research_info")

        if not research_info_list:
            logger.debug("No research info found for guild %s", self.id)
            self.lab_research = []
            return

        self.lab_research = [
            GuildLabResearchInfo(
                research_id=info["research_id"], work_amount=info["work_amount"]
            )
            for info in research_info_list
        ]

    def update_lab_research(self, updated_research: List[GuildLabResearchInfo]):
        if not self._lab_raw_data:
            logger.error(
                "Cannot update lab research, raw lab data not loaded for guild %s",
                self.id,
            )
            return

        new_research_info_list = [
            {"research_id": item.research_id, "work_amount": item.work_amount}
            for item in updated_research
        ]

        self._lab_raw_data["research_info"] = new_research_info_list
        self.lab_research = updated_research

    def update_from(self, guildDTO: GuildDTO):
        logger.debug("%s <= %s", self.id, guildDTO.name or "Unnamed GuildDTO")
        if guildDTO.name:
            self.name = guildDTO.name
        if guildDTO.bases:
            for base_id, base_dto in guildDTO.bases.items():
                if base_id in self.bases:
                    self.bases[base_id].update_from(base_dto)
                else:
                    logger.warning(
                        "Base %s not found in guild %s, adding new base",
                        base_id,
                        self.id,
                    )
        else:
            logger.warning("No bases found in GuildDTO for guild %s", self.id)

        if guildDTO.guild_chest and self.guild_chest is not None:
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
