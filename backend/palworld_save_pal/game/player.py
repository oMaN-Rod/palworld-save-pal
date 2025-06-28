from datetime import datetime, timedelta
from typing import Any, Dict, List, Optional, Union
from uuid import UUID
import uuid
from pydantic import BaseModel, ConfigDict, Field, PrivateAttr, computed_field

from palworld_save_tools.gvas import GvasFile

from palworld_save_pal.game.character_container import (
    CharacterContainer,
    CharacterContainerType,
)
from palworld_save_pal.game.guild import Guild
from palworld_save_pal.game.map import WorldMapPoint
from palworld_save_pal.game.pal import Pal
from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.game.item_container import ItemContainer, ItemContainerType
from palworld_save_pal.game.pal_objects import ArrayType, PalGender, PalObjects
from palworld_save_pal.utils.dict import safe_remove
from palworld_save_pal.utils.uuid import are_equal_uuids
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class PlayerGvasFiles(BaseModel):
    sav: GvasFile
    dps: Optional[GvasFile] = None

    model_config = ConfigDict(arbitrary_types_allowed=True)


class PlayerDTO(BaseModel):
    uid: UUID
    nickname: str
    level: int
    exp: int
    hp: int = 5000
    stomach: float = 100.0
    sanity: float = 100.0
    status_point_list: Dict[str, int] = Field(default_factory=dict)
    ext_status_point_list: Dict[str, int] = Field(default_factory=dict)
    instance_id: Optional[UUID] = Field(default=None)
    guild_id: Optional[UUID] = Field(default=None)
    pal_box_id: Optional[UUID] = Field(default=None)
    otomo_container_id: Optional[UUID] = Field(default=None)
    common_container: Optional[ItemContainer] = Field(default=None)
    essential_container: Optional[ItemContainer] = Field(default=None)
    weapon_load_out_container: Optional[ItemContainer] = Field(default=None)
    player_equipment_armor_container: Optional[ItemContainer] = Field(default=None)
    food_equip_container: Optional[ItemContainer] = Field(default=None)
    technologies: List[str] = Field(default_factory=list)
    technology_points: int = 0
    boss_technology_points: int = 0


class Player(BaseModel):
    _guild: Optional[Guild] = PrivateAttr(default=None)
    _player_gvas_files: PlayerGvasFiles
    _save_data: Dict[str, Any]
    _inventory_info: Dict[str, Any]
    _dynamic_item_save_data: Dict[str, Any]
    _character_save: Dict[str, Any]
    _save_parameter: Dict[str, Any]
    _dps: Optional[Dict[int, Pal]] = PrivateAttr(default=None)

    pals: Optional[Dict[UUID, Pal]] = Field(default_factory=dict)
    common_container: Optional[ItemContainer] = Field(default=None)
    essential_container: Optional[ItemContainer] = Field(default=None)
    weapon_load_out_container: Optional[ItemContainer] = Field(default=None)
    player_equipment_armor_container: Optional[ItemContainer] = Field(default=None)
    food_equip_container: Optional[ItemContainer] = Field(default=None)
    pal_box: Optional[CharacterContainer] = Field(default=None)
    party: Optional[CharacterContainer] = Field(default=None)

    def __init__(
        self,
        gvas_files: PlayerGvasFiles = None,
        item_container_save_data: Dict[str, Any] = None,
        dynamic_item_save_data: Dict[str, Any] = None,
        character_container_save_data: Dict[str, Any] = None,
        character_save_parameter: Dict[str, Any] = None,
        guild: Optional[Guild] = None,
        **kwargs,
    ):
        super().__init__(**kwargs)
        if (
            gvas_files is not None
            and item_container_save_data is not None
            and dynamic_item_save_data is not None
            and character_container_save_data is not None
            and character_save_parameter is not None
        ):
            self._character_save = character_save_parameter
            self._save_parameter = PalObjects.get_nested(
                self._character_save,
                "value",
                "RawData",
                "value",
                "object",
                "SaveParameter",
                "value",
            )
            self._player_gvas_files = gvas_files
            self._save_data = PalObjects.get_value(
                self._player_gvas_files.sav.properties["SaveData"]
            )
            self._guild = guild
            self._load_inventory(item_container_save_data, dynamic_item_save_data)
            self._load_pal_box(character_container_save_data)
            self._load_otomo_container(character_container_save_data)
            if self._player_gvas_files.dps:
                self._load_dps()

    @computed_field
    def guild_id(self) -> Optional[UUID]:
        return self._guild.id if self._guild else None

    @computed_field
    def uid(self) -> UUID:
        return PalObjects.get_guid(self._character_save["key"]["PlayerUId"])

    @computed_field
    def instance_id(self) -> Optional[UUID]:
        return PalObjects.get_guid(self._character_save["key"]["InstanceId"])

    @computed_field
    def nickname(self) -> str:
        if "NickName" not in self._save_parameter:
            return f"🥷 ({str(self.uid).split('-')[0]})"
        else:
            return PalObjects.get_value(self._save_parameter["NickName"])

    @nickname.setter
    def nickname(self, value: str):
        default_pattern = f"🥷 ({str(self.uid).split('-')[0]})"
        if value == default_pattern:
            safe_remove(self._save_parameter, "NickName")
            return
        PalObjects.set_value(self._save_parameter["NickName"], value=value)

    @computed_field
    def level(self) -> int:
        return (
            PalObjects.get_byte_property(self._save_parameter["Level"])
            if "Level" in self._save_parameter
            else 1
        )

    @level.setter
    def level(self, value: int):
        self._save_parameter["Level"] = PalObjects.ByteProperty(value)

    @computed_field
    def technologies(self) -> List[str]:
        return PalObjects.get_array_property(
            self._save_data["UnlockedRecipeTechnologyNames"]
        )

    @technologies.setter
    def technologies(self, value: List[str]):
        self._save_data["UnlockedRecipeTechnologyNames"] = (
            PalObjects.ArrayPropertyValues(ArrayType.NAME_PROPERTY, values=value)
        )

    @computed_field
    def technology_points(self) -> int:
        return (
            PalObjects.get_value(self._save_data["TechnologyPoint"])
            if "TechnologyPoint" in self._save_data
            else 0
        )

    @technology_points.setter
    def technology_points(self, value: int):
        self._save_data["TechnologyPoint"] = PalObjects.IntProperty(value)

    @computed_field
    def boss_technology_points(self) -> int:
        return (
            PalObjects.get_value(self._save_data["bossTechnologyPoint"])
            if "bossTechnologyPoint" in self._save_data
            else 0
        )

    @boss_technology_points.setter
    def boss_technology_points(self, value: int):
        self._save_data["bossTechnologyPoint"] = PalObjects.IntProperty(value)

    @computed_field
    def exp(self) -> int:
        return (
            PalObjects.get_value(self._save_parameter["Exp"])
            if "Exp" in self._save_parameter
            else 0
        )

    @exp.setter
    def exp(self, value: int):
        self._save_parameter["Exp"] = PalObjects.Int64Property(value)

    @computed_field
    def hp(self) -> int:
        if "HP" in self._save_parameter:
            self._save_parameter["Hp"] = self._save_parameter.pop("HP")
        return (
            PalObjects.get_fixed_point64(self._save_parameter["Hp"])
            if "Hp" in self._save_parameter
            else 0
        )

    @hp.setter
    def hp(self, value: int):
        self._save_parameter["Hp"] = PalObjects.FixedPoint64(value)

    @computed_field
    def stomach(self) -> float:
        return (
            PalObjects.get_value(self._save_parameter["FullStomach"])
            if "FullStomach" in self._save_parameter
            else 150.0
        )

    @stomach.setter
    def stomach(self, value: float):
        self._save_parameter["FullStomach"] = PalObjects.FloatProperty(value)

    @computed_field
    def sanity(self) -> float:
        return (
            PalObjects.get_value(self._save_parameter["SanityValue"], 100.0)
            if "SanityValue" in self._save_parameter
            else 100.0
        )

    @sanity.setter
    def sanity(self, value: float):
        self._save_parameter["SanityValue"] = PalObjects.FloatProperty(value)

    @computed_field
    def status_point_list(self) -> Dict[str, int]:
        status_point_list = PalObjects.get_array_property(
            self._save_parameter["GotStatusPointList"]
        )
        return {
            PalObjects.StatusNameMap[
                PalObjects.get_value(item["StatusName"])
            ]: PalObjects.get_value(item["StatusPoint"])
            for item in status_point_list
        }

    @status_point_list.setter
    def status_point_list(self, value: Dict[str, int]):
        status_point_list = PalObjects.get_array_property(
            self._save_parameter["GotStatusPointList"]
        )
        reverse_status_map = {v: k for k, v in PalObjects.StatusNameMap.items()}
        for status_name, point_value in value.items():
            japanese_name = reverse_status_map[status_name]
            for item in status_point_list:
                if PalObjects.get_value(item["StatusName"]) == japanese_name:
                    PalObjects.set_value(item["StatusPoint"], point_value)
                    break

    @computed_field
    def ext_status_point_list(self) -> Dict[str, int]:
        ext_status_point_list = PalObjects.get_array_property(
            self._save_parameter["GotExStatusPointList"]
        )
        return {
            PalObjects.ExStatusNameMap[
                PalObjects.get_value(item["StatusName"])
            ]: PalObjects.get_value(item["StatusPoint"])
            for item in ext_status_point_list
        }

    @ext_status_point_list.setter
    def ext_status_point_list(self, value: Dict[str, int]):
        ext_status_point_list = PalObjects.get_array_property(
            self._save_parameter["GotExStatusPointList"]
        )
        reverse_ex_status_map = {v: k for k, v in PalObjects.ExStatusNameMap.items()}
        for status_name, point_value in value.items():
            japanese_name = reverse_ex_status_map[status_name]
            for item in ext_status_point_list:
                if PalObjects.get_value(item["StatusName"]) == japanese_name:
                    PalObjects.set_value(item["StatusPoint"], point_value)
                    break

    @computed_field
    def pal_box_id(self) -> Optional[UUID]:
        return PalObjects.get_guid(
            PalObjects.get_nested(
                self._player_gvas_files.sav.properties["SaveData"],
                "value",
                "PalStorageContainerId",
                "value",
                "ID",
            )
        )

    @computed_field
    def otomo_container_id(self) -> Optional[UUID]:
        return PalObjects.get_guid(
            PalObjects.get_nested(
                self._player_gvas_files.sav.properties["SaveData"],
                "value",
                "OtomoCharacterContainerId",
                "value",
                "ID",
            )
        )

    @computed_field
    def location(self) -> Optional[WorldMapPoint]:
        last_location = PalObjects.get_value(self._save_parameter["LastJumpedLocation"])
        return (
            WorldMapPoint(
                x=last_location["x"],
                y=last_location["y"],
                z=last_location["z"],
            )
            if "LastJumpedLocation" in self._save_parameter
            else None
        )

    @computed_field
    def last_online_time(self) -> datetime:
        ticks = PalObjects.get_value(
            self._player_gvas_files.sav.properties["Timestamp"]
        )
        seconds = ticks / 10000000
        days = seconds // 86400
        seconds_remainder = seconds % 86400
        base_date = datetime(1, 1, 1)
        return base_date + timedelta(days=days, seconds=seconds_remainder)

    @computed_field
    def dps(self) -> Dict[int, Pal]:
        return self._dps

    @property
    def character_save(self) -> Dict[str, Any]:
        return self._character_save

    @property
    def save_data(self) -> Dict[str, Any]:
        return {
            "character_save": {**self._character_save},
            "gvas_properties": {**self._player_gvas_files.sav.properties},
            "dps_gvas_properties": (
                {**self._player_gvas_files.dps.properties}
                if self._player_gvas_files.dps
                else {}
            ),
        }

    def add_pal(
        self,
        character_id: str,
        nickname: str,
        container_id: UUID,
        storage_slot: Union[int | None] = None,
    ) -> Optional[Pal]:
        new_pal_id = uuid.uuid4()
        container = (
            self.pal_box
            if are_equal_uuids(container_id, self.pal_box_id)
            else self.party
        )
        slot_idx = container.add_pal(new_pal_id, storage_slot)
        if slot_idx is None:
            return

        new_pal_data = PalObjects.PalSaveParameter(
            character_id=character_id,
            instance_id=new_pal_id,
            owner_uid=self.uid,
            container_id=container_id,
            slot_idx=slot_idx,
            group_id=self._guild.id if isinstance(self._guild, Guild) else None,
            nickname=nickname,
        )
        new_pal = Pal(new_pal_data)
        new_pal.hp = new_pal.max_hp
        if not self.pals:
            self.pals = {}
        self.pals[new_pal_id] = new_pal
        if isinstance(self._guild, Guild):
            self._guild.add_pal(new_pal_id)
        return new_pal

    def add_dps_pal(
        self,
        character_id: str,
        nickname: str,
        storage_slot: Optional[int] = None,
    ) -> Optional[Pal]:
        slot_idx = (
            storage_slot
            if storage_slot is not None
            else self._find_first_empty_dps_slot()
        )
        pal_data = PalObjects.get_array_property(
            self._player_gvas_files.dps.properties["SaveParameterArray"]
        )[slot_idx]

        pal = Pal(data=pal_data, dps=True)
        pal.reset()
        pal.owner_uid = self.uid
        pal.instance_id = uuid.uuid4()
        pal.character_id = character_id
        pal.nickname = nickname
        pal.filtered_nickname = nickname
        pal.storage_id = self.pal_box_id
        pal.storage_slot = 0
        pal.gender = PalGender.FEMALE
        pal.populate_status_point_lists()
        pal.hp = pal.max_hp
        self._dps[slot_idx] = pal
        return slot_idx, pal

    def update_dps_pal(self, index: int, pal_dto: PalDTO):
        pal = self._dps[index]
        pal.update_from(pal_dto)

    def move_pal(self, pal_id: UUID, container_id: UUID):
        pal = self.pals[pal_id]
        if are_equal_uuids(container_id, self.pal_box_id):
            source_container = self.party
            target_container = self.pal_box
        elif are_equal_uuids(container_id, self.otomo_container_id):
            source_container = self.pal_box
            target_container = self.party
        else:
            logger.error("Invalid container id %s", container_id)
            return
        slot_idx = target_container.add_pal(pal_id)
        if slot_idx is None:
            return
        source_container.remove_pal(pal_id)
        pal.storage_id = container_id
        pal.storage_slot = slot_idx
        return pal

    def clone_pal(self, pal: PalDTO) -> Optional[Pal]:
        new_pal_id = uuid.uuid4()
        storage_slot = self.pal_box.add_pal(new_pal_id)
        if not storage_slot:
            return
        existing_pal = self.pals[pal.instance_id]
        nickname = pal.nickname if pal.nickname else pal.character_id
        new_pal = existing_pal.clone(
            new_pal_id, self.pal_box_id, storage_slot, nickname
        )
        self.pals[new_pal_id] = new_pal
        if isinstance(self._guild, Guild):
            self._guild.add_pal(new_pal_id)
        return new_pal

    def clone_dps_pal(self, palDTO: PalDTO) -> Optional[Pal]:
        if not self._player_gvas_files.dps:
            logger.error("No dps gvas found for player %s", self.uid)
            return None
        slot_idx = self._find_first_empty_dps_slot()
        if slot_idx is None:
            logger.error("No empty DPS slots available for player %s", self.uid)
            return None

        pal_data = PalObjects.get_array_property(
            self._player_gvas_files.dps.properties["SaveParameterArray"]
        )[slot_idx]
        pal = Pal(data=pal_data, dps=True)
        pal.update_from(palDTO)
        pal.instance_id = uuid.uuid4()
        pal.populate_status_point_lists()
        pal.hp = pal.max_hp
        self._dps[slot_idx] = pal
        return slot_idx, pal

    def delete_pal(self, pal_id: UUID):
        self.pals.pop(pal_id)
        self.pal_box.remove_pal(pal_id)
        self.party.remove_pal(pal_id)
        if isinstance(self._guild, Guild):
            self._guild.delete_character_handle(pal_id)

    def delete_dps_pals(self, pal_indexes: List[int]) -> None:
        if not self._player_gvas_files.dps:
            logger.error("No dps gvas found for player %s", self.uid)
            return
        for index in sorted(pal_indexes, reverse=True):
            if index in self._dps:
                pal = self._dps[index]
                pal.reset()

    def update_from(self, other_player: PlayerDTO):
        logger.debug(
            "Updating player %s from player %s", self.nickname, other_player.nickname
        )
        data = other_player.model_dump()
        logger.debug("Data to update from: %s", data.keys())
        for key, value in data.items():
            match key:
                case (
                    "level"
                    | "exp"
                    | "status_point_list"
                    | "ext_status_point_list"
                    | "hp"
                    | "stomach"
                    | "nickname"
                    | "sanity"
                    | "technologies"
                    | "technology_points"
                    | "boss_technology_points"
                ):
                    setattr(self, key, value)
                case "common_container":
                    self.common_container.update_from(value)
                case "essential_container":
                    self.essential_container.update_from(value)
                case "weapon_load_out_container":
                    self.weapon_load_out_container.update_from(value)
                case "player_equipment_armor_container":
                    self.player_equipment_armor_container.update_from(value)
                case "food_equip_container":
                    self.food_equip_container.update_from(value)
                case _:
                    logger.debug("Ignoring key %s", key)
                    continue

    def _load_pal_box(self, character_container_save_data: Dict[str, Any]):
        self.pal_box = CharacterContainer(
            id=self.pal_box_id,
            player_uid=self.uid,
            type=CharacterContainerType.PAL_BOX,
            character_container_save_data=character_container_save_data,
        )

    def _load_otomo_container(self, character_container_save_data: Dict[str, Any]):
        self.party = CharacterContainer(
            id=self.otomo_container_id,
            player_uid=self.uid,
            type=CharacterContainerType.PARTY,
            character_container_save_data=character_container_save_data,
        )

    def _load_common_container(
        self,
        inventory_info: Dict[str, Any],
        item_container_save_data: Dict[str, Any],
        dynamic_item_save_data: Dict[str, Any],
    ):
        common_container_id = PalObjects.get_guid(
            PalObjects.get_nested(inventory_info, "CommonContainerId", "value", "ID")
        )
        self.common_container = ItemContainer(
            id=common_container_id,
            type=ItemContainerType.COMMON,
            item_container_save_data=item_container_save_data,
            dynamic_item_save_data=dynamic_item_save_data,
        )

    def _load_essential_container(
        self,
        inventory_info: Dict[str, Any],
        item_container_save_data: Dict[str, Any],
        dynamic_item_save_data: Dict[str, Any],
    ):
        essential_container_id = PalObjects.get_guid(
            PalObjects.get_nested(inventory_info, "EssentialContainerId", "value", "ID")
        )
        self.essential_container = ItemContainer(
            id=essential_container_id,
            type=ItemContainerType.ESSENTIAL,
            item_container_save_data=item_container_save_data,
            dynamic_item_save_data=dynamic_item_save_data,
        )

    def _load_weapon_load_out_container(
        self,
        inventory_info: Dict[str, Any],
        item_container_save_data: Dict[str, Any],
        dynamic_item_save_data: Dict[str, Any],
    ):
        weapon_load_out_container_id = PalObjects.get_guid(
            PalObjects.get_nested(
                inventory_info, "WeaponLoadOutContainerId", "value", "ID"
            )
        )
        self.weapon_load_out_container = ItemContainer(
            id=weapon_load_out_container_id,
            type=ItemContainerType.WEAPON,
            item_container_save_data=item_container_save_data,
            dynamic_item_save_data=dynamic_item_save_data,
        )

    def _load_player_equipment_armor_container(
        self,
        inventory_info: Dict[str, Any],
        item_container_save_data: Dict[str, Any],
        dynamic_item_save_data: Dict[str, Any],
    ):
        player_equipment_armor_container_id = PalObjects.get_guid(
            PalObjects.get_nested(
                inventory_info, "PlayerEquipArmorContainerId", "value", "ID"
            )
        )
        self.player_equipment_armor_container = ItemContainer(
            id=player_equipment_armor_container_id,
            type=ItemContainerType.ARMOR,
            item_container_save_data=item_container_save_data,
            dynamic_item_save_data=dynamic_item_save_data,
        )

    def _load_food_equip_container(
        self,
        inventory_info: Dict[str, Any],
        item_container_save_data: Dict[str, Any],
        dynamic_item_save_data: Dict[str, Any],
    ):
        food_equip_container_id = PalObjects.get_guid(
            PalObjects.get_nested(inventory_info, "FoodEquipContainerId", "value", "ID")
        )
        self.food_equip_container = ItemContainer(
            id=food_equip_container_id,
            type=ItemContainerType.FOOD,
            item_container_save_data=item_container_save_data,
            dynamic_item_save_data=dynamic_item_save_data,
        )

    def _load_inventory(
        self,
        item_container_save_data: Dict[str, Any],
        dynamic_item_save_data: Dict[str, Any],
    ):
        if "inventoryInfo" in self._save_data:
            logger.debug(
                "Converting inventory info to new format for player (%s) %s",
                self.uid,
                self.nickname,
            )
            self._save_data["InventoryInfo"] = self._save_data.pop("inventoryInfo")

        inventory_info = PalObjects.get_value(self._save_data["InventoryInfo"])

        if not inventory_info:
            logger.error("No inventory info found for player %s", self.uid)
            return

        logger.debug("Loading storage for player %s", self.nickname)
        self._load_common_container(
            inventory_info, item_container_save_data, dynamic_item_save_data
        )
        self._load_essential_container(
            inventory_info, item_container_save_data, dynamic_item_save_data
        )
        self._load_weapon_load_out_container(
            inventory_info, item_container_save_data, dynamic_item_save_data
        )
        self._load_player_equipment_armor_container(
            inventory_info, item_container_save_data, dynamic_item_save_data
        )
        self._load_food_equip_container(
            inventory_info, item_container_save_data, dynamic_item_save_data
        )

    def _find_first_empty_dps_slot(self) -> Optional[int]:
        if not self._player_gvas_files.dps:
            return None
        for index, entry in enumerate(
            PalObjects.get_array_property(
                self._player_gvas_files.dps.properties["SaveParameterArray"]
            )
        ):
            save_parameter = PalObjects.get_value(entry["SaveParameter"])
            character_id = (
                PalObjects.get_value(save_parameter["CharacterID"])
                if "CharacterID" in save_parameter
                else None
            )
            if character_id is None or character_id == "None":
                logger.debug(
                    "Found empty DPS slot at index %s for player %s (%s)",
                    index,
                    self.nickname,
                    str(self.uid),
                )
                return index
        return None

    def _load_dps(self) -> None:
        self._dps = {}
        if not self._player_gvas_files.dps:
            return
        for index, entry in enumerate(
            PalObjects.get_array_property(
                self._player_gvas_files.dps.properties["SaveParameterArray"]
            )
        ):
            pal = Pal(data=entry, dps=True)
            if pal.character_id != "None":
                self._dps[index] = pal
