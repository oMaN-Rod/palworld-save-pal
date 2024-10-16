from typing import Any, Dict, List, Optional
from uuid import UUID
import uuid
from pydantic import BaseModel, Field, PrivateAttr

from palworld_save_tools.gvas import GvasFile

from palworld_save_pal.save_file.character_container import (
    CharacterContainer,
    CharacterContainerType,
)
from palworld_save_pal.save_file.guild import Guild
from palworld_save_pal.save_file.pal import Pal
from palworld_save_pal.save_file.item_container import ItemContainer, ItemContainerType
from palworld_save_pal.save_file.pal_objects import PalObjects
from palworld_save_pal.save_file.utils import are_equal_uuids
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class Player(BaseModel):
    uid: UUID
    nickname: str
    level: int
    exp: int
    instance_id: Optional[UUID] = Field(default=None)
    guild: Optional[Guild] = Field(default=None)

    pals: Optional[Dict[UUID, Pal]] = Field(default_factory=dict)
    pal_box_id: Optional[UUID] = Field(default=None)
    otomo_container_id: Optional[UUID] = Field(default=None)

    common_container: Optional[ItemContainer] = Field(default=None)
    essential_container: Optional[ItemContainer] = Field(default=None)
    weapon_load_out_container: Optional[ItemContainer] = Field(default=None)
    player_equipment_armor_container: Optional[ItemContainer] = Field(default=None)
    food_equip_container: Optional[ItemContainer] = Field(default=None)

    _pal_box: Optional[CharacterContainer] = PrivateAttr(default=None)
    _party: Optional[CharacterContainer] = PrivateAttr(default=None)
    _player_gvas_file: Optional[GvasFile] = PrivateAttr(default=None)
    _character_save_parameter: Optional[Dict[str, Any]] = PrivateAttr(default=None)

    def __init__(
        self,
        gvas_file=None,
        item_container_save_data: Dict[str, Any] = None,
        dynamic_item_save_data: Dict[str, Any] = None,
        character_container_save_data: Dict[str, Any] = None,
        character_save_parameter: Dict[str, Any] = None,
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
            self._character_save_parameter = character_save_parameter
            self._player_gvas_file = gvas_file
            self._load_player_data()
            self._load_inventory(item_container_save_data, dynamic_item_save_data)
            self._load_pal_box(character_container_save_data)
            self._load_otomo_container(character_container_save_data)

    def add_pal(self, pal_code_name: str, nickname: str, container_id: UUID):
        new_pal_id = uuid.uuid4()
        container = (
            self._pal_box
            if are_equal_uuids(container_id, self.pal_box_id)
            else self._party
        )
        slot_idx = container.add_pal(new_pal_id)
        if slot_idx is None:
            return

        new_pal_data = PalObjects.PalSaveParameter(
            code_name=pal_code_name,
            instance_id=new_pal_id,
            owner_uid=self.uid,
            container_id=container_id,
            slot_idx=slot_idx,
            group_id=self.guild.id if isinstance(self.guild, Guild) else None,
            nickname=nickname,
        )
        new_pal = Pal(new_pal_data)

        if not self.pals:
            self.pals = {}
        self.pals[new_pal_id] = new_pal
        if isinstance(self.guild, Guild):
            self.guild.add_pal(new_pal_id)
        return new_pal, new_pal_data

    def move_pal(self, pal_id: UUID, container_id: UUID):
        pal = self.pals[pal_id]
        if are_equal_uuids(container_id, self.pal_box_id):
            source_container = self._party
            target_container = self._pal_box
        elif are_equal_uuids(container_id, self.otomo_container_id):
            source_container = self._pal_box
            target_container = self._party
        else:
            logger.error("Invalid container id %s", container_id)
            return
        slot_idx = target_container.add_pal(pal_id)
        if slot_idx is None:
            return
        source_container.remove_pal(pal_id)
        pal.storage_id = container_id
        pal.storage_slot = slot_idx
        pal.update()
        return pal

    def clone_pal(self, pal: Pal):
        new_pal_id = uuid.uuid4()
        slot_idx = self._pal_box.add_pal(new_pal_id)
        if not slot_idx:
            return
        existing_pal = self.pals[pal.instance_id]
        nickname = pal.nickname if pal.nickname else f"[New] {pal.character_id}"
        new_pal = existing_pal.clone(new_pal_id, slot_idx, nickname)
        self.pals[new_pal_id] = new_pal
        if isinstance(self.guild, Guild):
            self.guild.add_pal(new_pal_id)
        return new_pal

    def delete_pal(self, pal_id: UUID):
        self.pals.pop(pal_id)
        self._pal_box.remove_pal(pal_id)
        if isinstance(self.guild, Guild):
            self.guild.remove_pal(pal_id)

    def update_from(self, other_player: "Player"):
        logger.debug(
            "Updating player %s from player %s", self.nickname, other_player.nickname
        )
        data = other_player.model_dump()
        logger.debug("Data to update from: %s", data.keys())
        for key, value in data.items():
            match key:
                case "pals":
                    continue
                case "level":
                    self.level = value
                    if "Level" in self._character_save_parameter:
                        PalObjects.set_byte_property(
                            self._character_save_parameter["Level"], value=value
                        )
                    else:
                        self._character_save_parameter["Level"] = (
                            PalObjects.ByteProperty(value)
                        )
                case "exp":
                    self.exp = value
                    if "Exp" in self._character_save_parameter:
                        PalObjects.set_value(
                            self._character_save_parameter["Exp"], value=value
                        )
                    else:
                        self._character_save_parameter["Exp"] = (
                            PalObjects.Int64Property(value)
                        )
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

    def _load_player_data(self):
        player_save_data = PalObjects.get_value(
            self._player_gvas_file.properties["SaveData"]
        )
        self.instance_id = PalObjects.get_guid(
            PalObjects.get_nested(
                player_save_data, "IndividualId", "value", "InstanceId"
            )
        )

    def _load_pal_box(self, character_container_save_data: Dict[str, Any]):
        player_save_data = PalObjects.get_value(
            self._player_gvas_file.properties["SaveData"]
        )
        self.pal_box_id = PalObjects.get_guid(
            PalObjects.get_nested(
                player_save_data, "PalStorageContainerId", "value", "ID"
            )
        )
        self._pal_box = CharacterContainer(
            id=self.pal_box_id,
            type=CharacterContainerType.PAL_BOX,
            character_container_save_data=character_container_save_data,
        )

    def _load_otomo_container(self, character_container_save_data: Dict[str, Any]):
        player_save_data = PalObjects.get_value(
            self._player_gvas_file.properties["SaveData"]
        )
        self.otomo_container_id = PalObjects.get_guid(
            PalObjects.get_nested(
                player_save_data, "OtomoCharacterContainerId", "value", "ID"
            )
        )
        self._party = CharacterContainer(
            id=self.otomo_container_id,
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
        player_save_data = PalObjects.get_value(
            self._player_gvas_file.properties["SaveData"]
        )
        inventory_info = PalObjects.get_value(player_save_data["InventoryInfo"])

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
