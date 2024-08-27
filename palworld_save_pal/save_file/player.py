from typing import Any, Dict, Optional
from uuid import UUID
from pydantic import BaseModel, Field, PrivateAttr

from palworld_save_tools.gvas import GvasFile

from palworld_save_pal.save_file.pal import Pal
from palworld_save_pal.save_file.item_container import ItemContainer
from palworld_save_pal.save_file.pal_objects import PalObjects
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class Player(BaseModel):
    uid: UUID
    nickname: str
    level: int
    pals: Optional[Dict[UUID, Pal]] = Field(default_factory=dict)
    common_container: Optional[ItemContainer] = Field(default=None)
    essential_container: Optional[ItemContainer] = Field(default=None)
    weapon_load_out_container: Optional[ItemContainer] = Field(default=None)
    player_equipment_armor_container: Optional[ItemContainer] = Field(default=None)
    food_equip_container: Optional[ItemContainer] = Field(default=None)

    _player_gvas_file: Optional[GvasFile] = PrivateAttr(default=None)

    def __init__(
        self,
        gvas_file=None,
        item_container_save_data: Dict[str, Any] = None,
        dynamic_item_save_data: Dict[str, Any] = None,
        **kwargs,
    ):
        super().__init__(**kwargs)
        if (
            gvas_file is not None
            and item_container_save_data is not None
            and dynamic_item_save_data is not None
        ):
            self._player_gvas_file = gvas_file
            self.load_storage(item_container_save_data, dynamic_item_save_data)

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
            id=common_container_id, type="CommonContainer"
        ).get_items(item_container_save_data, dynamic_item_save_data)

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
            id=essential_container_id, type="EssentialContainer"
        ).get_items(item_container_save_data, dynamic_item_save_data)

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
            id=weapon_load_out_container_id, type="WeaponLoadOutContainer"
        ).get_items(item_container_save_data, dynamic_item_save_data)

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
        ).get_items(item_container_save_data, dynamic_item_save_data)

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
            id=food_equip_container_id, type="FoodEquipContainer"
        ).get_items(item_container_save_data, dynamic_item_save_data)

    def load_storage(
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

    def update(
        self,
        item_container_save_data: Dict[str, Any],
        dynamic_item_save_data: Dict[str, Any],
    ):
        self.update_storage(item_container_save_data, dynamic_item_save_data)

    def update_storage(
        self,
        item_container_save_data: Dict[str, Any],
        dynamic_item_save_data: Dict[str, Any],
    ):
        self.common_container.set_items(
            item_container_save_data, dynamic_item_save_data
        )
        self.essential_container.set_items(
            item_container_save_data, dynamic_item_save_data
        )
        self.weapon_load_out_container.set_items(
            item_container_save_data, dynamic_item_save_data
        )
        self.player_equipment_armor_container.set_items(
            item_container_save_data, dynamic_item_save_data
        )
        self.food_equip_container.set_items(
            item_container_save_data, dynamic_item_save_data
        )

    def update_from(self, other_player: "Player"):
        data = other_player.model_dump()
        for key, value in data.items():
            match key:
                case "pals":
                    continue
                case "common_container":
                    self.common_container = ItemContainer(**value)
                case "essential_container":
                    self.essential_container = ItemContainer(**value)
                case "weapon_load_out_container":
                    self.weapon_load_out_container = ItemContainer(**value)
                case "player_equipment_armor_container":
                    self.player_equipment_armor_container = ItemContainer(**value)
                case "food_equip_container":
                    self.food_equip_container = ItemContainer(**value)
