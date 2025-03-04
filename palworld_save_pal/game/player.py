from typing import Any, Dict, List, Optional, Union
from uuid import UUID
import uuid
from pydantic import BaseModel, Field, PrivateAttr, computed_field

from palworld_save_tools.gvas import GvasFile

from palworld_save_pal.game.character_container import (
    CharacterContainer,
    CharacterContainerType,
)
from palworld_save_pal.game.guild import Guild, GuildDTO
from palworld_save_pal.game.pal import Pal, PalDTO
from palworld_save_pal.game.item_container import ItemContainer, ItemContainerType
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.dict import safe_remove
from palworld_save_pal.utils.uuid import are_equal_uuids
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


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
    _uid: UUID
    _nickname: str
    _level: int
    _technologies: List[str]
    _technology_points: int
    _boss_technology_points: int
    _exp: int
    _hp: int
    _stomach: float
    _sanity: float
    _status_point_list: Dict[str, int]
    _ext_status_point_list: Dict[str, int]
    _instance_id: UUID
    _pal_box_id: UUID
    _otomo_container_id: UUID

    _guild: Optional[Guild] = PrivateAttr(default=None)

    pals: Optional[Dict[UUID, Pal]] = Field(default_factory=dict)
    common_container: Optional[ItemContainer] = Field(default=None)
    essential_container: Optional[ItemContainer] = Field(default=None)
    weapon_load_out_container: Optional[ItemContainer] = Field(default=None)
    player_equipment_armor_container: Optional[ItemContainer] = Field(default=None)
    food_equip_container: Optional[ItemContainer] = Field(default=None)
    pal_box: Optional[CharacterContainer] = Field(default=None)
    party: Optional[CharacterContainer] = Field(default=None)

    _player_gvas_file: GvasFile
    _save_data: Dict[str, Any]
    _inventory_info: Dict[str, Any]
    _dynamic_item_save_data: Dict[str, Any]
    _character_save: Dict[str, Any]
    _save_parameter: Dict[str, Any]

    def __init__(
        self,
        gvas_file=None,
        item_container_save_data: Dict[str, Any] = None,
        dynamic_item_save_data: Dict[str, Any] = None,
        character_container_save_data: Dict[str, Any] = None,
        character_save_parameter: Dict[str, Any] = None,
        guild: Optional[Guild] = None,
        **kwargs,
    ):
        super().__init__(**kwargs)
        if (
            gvas_file is not None
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
            self._player_gvas_file = gvas_file
            self._save_data = PalObjects.get_value(
                self._player_gvas_file.properties["SaveData"]
            )
            self._guild = guild
            self._load_inventory(item_container_save_data, dynamic_item_save_data)
            self._load_pal_box(character_container_save_data)
            self._load_otomo_container(character_container_save_data)

    @computed_field
    def guild_id(self) -> Optional[UUID]:
        return self._guild.id if self._guild else None

    @computed_field
    def uid(self) -> UUID:
        self._uid = PalObjects.get_guid(self._character_save["key"]["PlayerUId"])
        return self._uid

    @computed_field
    def instance_id(self) -> Optional[UUID]:
        self._instance_id = PalObjects.get_guid(
            self._character_save["key"]["InstanceId"]
        )
        return self._instance_id

    @computed_field
    def nickname(self) -> str:
        if "NickName" not in self._save_parameter:
            self._nickname = f"🥷 ({str(self.uid).split("-")[0]})"
        else:
            self._nickname = PalObjects.get_value(self._save_parameter["NickName"])
        return self._nickname

    @nickname.setter
    def nickname(self, value: str):
        default_pattern = f"🥷 ({str(self.uid).split("-")[0]})"
        if value == default_pattern:
            safe_remove(self._save_parameter, "NickName")
            return

        self._nickname = value
        PalObjects.set_value(self._save_parameter["NickName"], value=value)

    @computed_field
    def level(self) -> int:
        self._level = (
            PalObjects.get_byte_property(self._save_parameter["Level"])
            if "Level" in self._save_parameter
            else 1
        )
        return self._level

    @level.setter
    def level(self, value: int):
        self._level = value
        if "Level" in self._save_parameter:
            PalObjects.set_byte_property(self._save_parameter["Level"], value=value)
        else:
            self._save_parameter["Level"] = PalObjects.ByteProperty(value)

    @computed_field
    def technologies(self) -> List[str]:
        self._technologies = PalObjects.get_array_property(
            self._save_data["UnlockedRecipeTechnologyNames"]
        )
        return self._technologies

    @technologies.setter
    def technologies(self, value: List[str]):
        self._technologies = value
        if "UnlockedRecipeTechnologyNames" in self._save_data:
            PalObjects.set_array_property(
                self._save_data["UnlockedRecipeTechnologyNames"],
                values=value,
            )
        else:
            self._save_data["UnlockedRecipeTechnologyNames"] = PalObjects.ArrayProperty(
                value=value
            )

    @computed_field
    def technology_points(self) -> int:
        self._technology_points = (
            PalObjects.get_value(self._save_data["TechnologyPoint"])
            if "TechnologyPoint" in self._save_data
            else 0
        )
        return self._technology_points

    @technology_points.setter
    def technology_points(self, value: int):
        self._technology_points = value
        if "TechnologyPoint" in self._save_data:
            PalObjects.set_value(self._save_data["TechnologyPoint"], value=value)
        else:
            self._save_data["TechnologyPoint"] = PalObjects.IntProperty(value)

    @computed_field
    def boss_technology_points(self) -> int:
        self._boss_technology_points = (
            PalObjects.get_value(self._save_data["bossTechnologyPoint"])
            if "bossTechnologyPoint" in self._save_data
            else 0
        )
        return self._boss_technology_points

    @boss_technology_points.setter
    def boss_technology_points(self, value: int):
        self._boss_technology_points = value
        if "bossTechnologyPoint" in self._save_data:
            PalObjects.set_value(self._save_data["bossTechnologyPoint"], value=value)
        else:
            self._save_data["bossTechnologyPoint"] = PalObjects.IntProperty(value)

    @computed_field
    def exp(self) -> int:
        self._exp = (
            PalObjects.get_value(self._save_parameter["Exp"])
            if "Exp" in self._save_parameter
            else 0
        )
        return self._exp

    @exp.setter
    def exp(self, value: int):
        self._exp = value
        if "Exp" in self._save_parameter:
            PalObjects.set_value(self._save_parameter["Exp"], value=value)
        else:
            self._save_parameter["Exp"] = PalObjects.Int64Property(value)

    @computed_field
    def hp(self) -> int:
        if "HP" in self._save_parameter:
            self._save_parameter["Hp"] = self._save_parameter.pop("HP")
        if "Hp" in self._save_parameter:
            self._hp = PalObjects.get_fixed_point64(self._save_parameter["Hp"])
        else:
            self._hp = 0
        return self._hp

    @hp.setter
    def hp(self, value: int):
        self._hp = value
        if "Hp" in self._save_parameter:
            PalObjects.set_fixed_point64(self._save_parameter["Hp"], value=value)
        else:
            self._save_parameter["Hp"] = PalObjects.FixedPoint64(value)

    @computed_field
    def stomach(self) -> float:
        self._stomach = (
            PalObjects.get_value(self._save_parameter["FullStomach"])
            if "FullStomach" in self._save_parameter
            else 150.0
        )
        return self._stomach

    @stomach.setter
    def stomach(self, value: float):
        self._stomach = value
        if "FullStomach" in self._save_parameter:
            PalObjects.set_value(self._save_parameter["FullStomach"], value=value)
        else:
            self._save_parameter["FullStomach"] = PalObjects.FloatProperty(value)

    @computed_field
    def sanity(self) -> float:
        self._sanity = (
            PalObjects.get_value(self._save_parameter["SanityValue"], 100.0)
            if "SanityValue" in self._save_parameter
            else 100.0
        )
        return self._sanity

    @sanity.setter
    def sanity(self, value: float):
        self._sanity = value
        if "SanityValue" in self._save_parameter:
            PalObjects.set_value(self._save_parameter["SanityValue"], value=value)
        else:
            self._save_parameter["SanityValue"] = PalObjects.FloatProperty(value)

    @computed_field
    def status_point_list(self) -> Dict[str, int]:
        status_point_list = PalObjects.get_array_property(
            self._save_parameter["GotStatusPointList"]
        )
        self._status_point_list = {
            PalObjects.StatusNameMap[
                PalObjects.get_value(item["StatusName"])
            ]: PalObjects.get_value(item["StatusPoint"])
            for item in status_point_list
        }
        return self._status_point_list

    @status_point_list.setter
    def status_point_list(self, value: Dict[str, int]):
        self._status_point_list = value
        status_point_list = PalObjects.get_array_property(
            self._save_parameter["GotStatusPointList"]
        )
        reverse_status_map = {v: k for k, v in PalObjects.StatusNameMap.items()}
        for status_name, point_value in self._status_point_list.items():
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
        self._ext_status_point_list = {
            PalObjects.ExStatusNameMap[
                PalObjects.get_value(item["StatusName"])
            ]: PalObjects.get_value(item["StatusPoint"])
            for item in ext_status_point_list
        }
        return self._ext_status_point_list

    @ext_status_point_list.setter
    def ext_status_point_list(self, value: Dict[str, int]):
        self._ext_status_point_list = value
        ext_status_point_list = PalObjects.get_array_property(
            self._save_parameter["GotExStatusPointList"]
        )
        reverse_ex_status_map = {v: k for k, v in PalObjects.ExStatusNameMap.items()}
        for status_name, point_value in self._ext_status_point_list.items():
            japanese_name = reverse_ex_status_map[status_name]
            for item in ext_status_point_list:
                if PalObjects.get_value(item["StatusName"]) == japanese_name:
                    PalObjects.set_value(item["StatusPoint"], point_value)
                    break

    @computed_field
    def pal_box_id(self) -> Optional[UUID]:
        self._pal_box_id = PalObjects.get_guid(
            PalObjects.get_nested(
                self._player_gvas_file.properties["SaveData"],
                "value",
                "PalStorageContainerId",
                "value",
                "ID",
            )
        )
        return self._pal_box_id

    @computed_field
    def otomo_container_id(self) -> Optional[UUID]:
        self._otomo_container_id = PalObjects.get_guid(
            PalObjects.get_nested(
                self._player_gvas_file.properties["SaveData"],
                "value",
                "OtomoCharacterContainerId",
                "value",
                "ID",
            )
        )
        return self._otomo_container_id

    @property
    def character_save(self) -> Dict[str, Any]:
        return self._character_save

    @property
    def save_data(self) -> Dict[str, Any]:
        return {
            "character_save": {**self._character_save},
            "gvas_properties": {**self._player_gvas_file.properties},
        }

    def add_pal(
        self,
        pal_code_name: str,
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
            character_id=pal_code_name,
            instance_id=new_pal_id,
            owner_uid=self.uid,
            container_id=container_id,
            slot_idx=slot_idx,
            group_id=self._guild.id if isinstance(self._guild, Guild) else None,
            nickname=nickname,
        )
        new_pal = Pal(new_pal_data)

        if not self.pals:
            self.pals = {}
        self.pals[new_pal_id] = new_pal
        if isinstance(self._guild, Guild):
            self._guild.add_pal(new_pal_id)
        return new_pal

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

    def delete_pal(self, pal_id: UUID):
        self.pals.pop(pal_id)
        self.pal_box.remove_pal(pal_id)
        self.party.remove_pal(pal_id)
        if isinstance(self._guild, Guild):
            self._guild.delete_pal(pal_id)

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