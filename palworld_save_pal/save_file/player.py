from typing import Any, Dict, List, Optional
from uuid import UUID
import uuid
from pydantic import BaseModel, Field, PrivateAttr

from palworld_save_tools.gvas import GvasFile

from palworld_save_pal.save_file.character_container import CharacterContainer
from palworld_save_pal.save_file.guild import Guild
from palworld_save_pal.save_file.pal import Pal
from palworld_save_pal.save_file.item_container import ItemContainer
from palworld_save_pal.save_file.pal_objects import PalObjects
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class Player(BaseModel):
    uid: UUID
    nickname: str
    level: int
    instance_id: Optional[UUID] = Field(default=None)
    pals: Optional[Dict[UUID, Pal]] = Field(default_factory=dict)
    common_container: Optional[ItemContainer] = Field(default=None)
    essential_container: Optional[ItemContainer] = Field(default=None)
    weapon_load_out_container: Optional[ItemContainer] = Field(default=None)
    player_equipment_armor_container: Optional[ItemContainer] = Field(default=None)
    food_equip_container: Optional[ItemContainer] = Field(default=None)
    guild: Optional[Guild] = Field(default=None)

    _pal_box: Optional[CharacterContainer] = PrivateAttr(default=None)
    _player_gvas_file: Optional[GvasFile] = PrivateAttr(default=None)

    def __init__(
        self,
        gvas_file=None,
        item_container_save_data: Dict[str, Any] = None,
        dynamic_item_save_data: Dict[str, Any] = None,
        character_container_save_data: Dict[str, Any] = None,
        **kwargs,
    ):
        super().__init__(**kwargs)
        if (
            gvas_file is not None
            and item_container_save_data is not None
            and dynamic_item_save_data is not None
            and character_container_save_data is not None
        ):
            self._player_gvas_file = gvas_file
            self._load_player_data()
            self._load_inventory(item_container_save_data, dynamic_item_save_data)
            self._load_pal_box(character_container_save_data)

    def add_pal(self, pal_code_name: str, nickname: str):
        new_pal_id = uuid.uuid4()
        slot_idx = self._pal_box.add_pal(new_pal_id)
        if slot_idx is None:
            return

        new_pal_data = PalObjects.PalSaveParameter(
            code_name=pal_code_name,
            instance_id=new_pal_id,
            owner_uid=self.uid,
            container_id=self._pal_box.id,
            slot_idx=slot_idx,
            group_id=self.guild.id if self.guild else None,
            nickname=nickname,
        )
        new_pal = Pal(new_pal_data)
        self.pals[new_pal_id] = new_pal
        if self.guild:
            self.guild.add_pal(new_pal_id)
        return new_pal, new_pal_data

    def clone_pal(self, pal: Pal):
        new_pal_id = uuid.uuid4()
        slot_idx = self._pal_box.add_pal(new_pal_id)
        if not slot_idx:
            return
        existing_pal = self.pals[pal.instance_id]
        nickname = f"🆕 {pal.nickname}" if pal.nickname else f"🆕 {pal.character_id}"
        new_pal = existing_pal.clone(new_pal_id, slot_idx, nickname)
        self.pals[new_pal_id] = new_pal
        if self.guild:
            self.guild.add_pal(new_pal_id)
        return new_pal

    def delete_pal(self, pal_id: UUID):
        self.pals.pop(pal_id)
        self._pal_box.delete_pal(pal_id)
        if self.guild:
            self.guild.remove_pal(pal_id)

    def update_from(self, other_player: "Player"):
        data = other_player.model_dump()
        for key, value in data.items():
            match key:
                case "pals":
                    continue
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
        self._update_storage()

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
        pal_storage_container_id = PalObjects.get_guid(
            PalObjects.get_nested(
                player_save_data, "PalStorageContainerId", "value", "ID"
            )
        )
        self._pal_box = CharacterContainer(
            id=pal_storage_container_id,
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
            type="CommonContainer",
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
            type="EssentialContainer",
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
            type="WeaponLoadOutContainer",
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
            type="PlayerEquipArmorContainer",
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
            type="FoodEquipContainer",
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
            # Older save file had inventoryInfo 🤷‍♂️
            inventory_info = PalObjects.get_value(player_save_data["inventoryInfo"])

        if not inventory_info:
            logger.error("No inventory info found for player %s", self.uid)
            return
        logger.info("Loading storage for player %s", self.nickname)
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

    def _update_storage(self):
        self.common_container.set_items()
        self.essential_container.set_items()
        self.weapon_load_out_container.set_items()
        self.player_equipment_armor_container.set_items()
        self.food_equip_container.set_items()
