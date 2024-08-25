import copy
import json
import os
from typing import Any, Dict, List, Optional
from uuid import UUID
import uuid
from pydantic import BaseModel, ConfigDict, PrivateAttr

from palworld_save_tools.archive import (
    UUID as ArchiveUUID,
    FArchiveReader,
    FArchiveWriter,
)
from palworld_save_tools.gvas import GvasFile
from palworld_save_tools.json_tools import CustomEncoder
from palworld_save_tools.palsav import compress_gvas_to_sav, decompress_sav_to_gvas
from palworld_save_tools.paltypes import (
    DISABLED_PROPERTIES,
    PALWORLD_CUSTOM_PROPERTIES,
    PALWORLD_TYPE_HINTS,
)

from palworld_save_pal.save_file.pal import Pal, PalSummary
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.save_file.dynamic_item import DynamicItem
from palworld_save_pal.save_file.item_container import ContainerSlot, ItemContainer
from palworld_save_pal.save_file.player import Player
from palworld_save_pal.save_file.utils import (
    safe_get,
    safe_remove,
    safe_set,
    is_valid_uuid,
    are_equal_uuids,
    is_empty_uuid,
)
from palworld_save_pal.save_file.empty_objects import get_empty_property, PropertyType

logger = create_logger(__name__)


def skip_decode(reader: FArchiveReader, type_name: str, size: int, path: str):
    if type_name == "ArrayProperty":
        array_type = reader.fstring()
        value = {
            "skip_type": type_name,
            "array_type": array_type,
            "id": reader.optional_guid(),
            "value": reader.read(size),
        }
    elif type_name == "MapProperty":
        key_type = reader.fstring()
        value_type = reader.fstring()
        _id = reader.optional_guid()
        value = {
            "skip_type": type_name,
            "key_type": key_type,
            "value_type": value_type,
            "id": _id,
            "value": reader.read(size),
        }
    elif type_name == "StructProperty":
        value = {
            "skip_type": type_name,
            "struct_type": reader.fstring(),
            "struct_id": reader.guid(),
            "id": reader.optional_guid(),
            "value": reader.read(size),
        }
    else:
        raise Exception(
            f"Expected ArrayProperty or MapProperty or StructProperty, got {type_name} in {path}"
        )
    return value


def skip_encode(writer: FArchiveWriter, property_type: str, properties: dict) -> int:
    if "skip_type" not in properties:
        if properties["custom_type"] in PALWORLD_CUSTOM_PROPERTIES is not None:
            return PALWORLD_CUSTOM_PROPERTIES[properties["custom_type"]][1](
                writer, property_type, properties
            )
        else:
            # Never be run to here
            return writer.property_inner(writer, property_type, properties)
    if property_type == "ArrayProperty":
        del properties["custom_type"]
        del properties["skip_type"]
        writer.fstring(properties["array_type"])
        writer.optional_guid(properties.get("id", None))
        writer.write(properties["value"])
        return len(properties["value"])
    elif property_type == "MapProperty":
        del properties["custom_type"]
        del properties["skip_type"]
        writer.fstring(properties["key_type"])
        writer.fstring(properties["value_type"])
        writer.optional_guid(properties.get("id", None))
        writer.write(properties["value"])
        return len(properties["value"])
    elif property_type == "StructProperty":
        del properties["custom_type"]
        del properties["skip_type"]
        writer.fstring(properties["struct_type"])
        writer.guid(properties["struct_id"])
        writer.optional_guid(properties.get("id", None))
        writer.write(properties["value"])
        return len(properties["value"])
    else:
        raise Exception(
            f"Expected ArrayProperty or MapProperty or StructProperty, got {property_type}"
        )


CUSTOM_PROPERTIES = {
    k: v for k, v in PALWORLD_CUSTOM_PROPERTIES.items() if k not in DISABLED_PROPERTIES
}
CUSTOM_PROPERTIES[".worldSaveData.MapObjectSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.FoliageGridSaveDataMap"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.MapObjectSpawnerInStageSaveData"] = (
    skip_decode,
    skip_encode,
)
CUSTOM_PROPERTIES[".worldSaveData.WorkSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.DungeonSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.EnemyCampSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.CharacterParameterStorageSaveData"] = (
    skip_decode,
    skip_encode,
)
CUSTOM_PROPERTIES[".worldSaveData.InvaderSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.DungeonPointMarkerSaveData"] = (
    skip_decode,
    skip_encode,
)
CUSTOM_PROPERTIES[".worldSaveData.GameTimeSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.OilrigSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.SupplySaveData"] = (skip_decode, skip_encode)


class SaveFile(BaseModel):
    name: str = ""
    size: int = 0

    model_config = ConfigDict(arbitrary_types_allowed=True)

    _gvas_file: Optional[GvasFile] = PrivateAttr(default=None)
    _player_gvas_files: Dict[UUID, GvasFile] = PrivateAttr(default_factory=dict)
    _players: Dict[UUID, Player] = PrivateAttr(default_factory=dict)
    _pals: Dict[UUID, PalSummary] = PrivateAttr(default_factory=dict)
    _character_save_parameter_map: List[Dict[str, Any]] = PrivateAttr(
        default_factory=dict
    )
    _item_container_save_data: List[Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _dynamic_item_save_data: List[Dict[str, Any]] = PrivateAttr(default_factory=dict)

    def get_pal(self, instance_id: UUID):
        for e in self._character_save_parameter_map:
            current_instance_id = safe_get(e, "key", "InstanceId", "value")
            if not self._is_player(e) and are_equal_uuids(
                current_instance_id, instance_id
            ):
                return Pal.create_safe(copy.deepcopy(e))
        return None

    def get_json(self, minify=False, allow_nan=True):
        logger.info("Converting %s to JSON", self.name)
        return json.dumps(
            self._gvas_file.dump(),
            indent=None if minify else "\t",
            cls=CustomEncoder,
            allow_nan=allow_nan,
        )

    def get_pals(self):
        return self._pals

    def get_players(self):
        return self._players

    def load_json(self, data: bytes):
        logger.info("Loading %s as JSON", self.name)
        self._gvas_file = GvasFile.load(json.loads(data))
        return self

    def load_gvas_files(self, level_sav: bytes, player_savs: Dict[str, bytes]):
        self.load_level_sav(level_sav)
        for player_id, player_data in player_savs.items():
            player_uuid = self._get_player_uuid(player_id)
            if not player_uuid:
                logger.warning(
                    "Player with ID %s not found in the save file.", player_id
                )
                continue
            raw_gvas, _ = decompress_sav_to_gvas(player_data)
            gvas_file = GvasFile.read(
                raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
            )
            self._player_gvas_files[player_uuid] = gvas_file
            self._update_player_storage(player_uuid)
        return self

    def load_level_sav(self, data: bytes):
        logger.info("Loading %s as GVAS", self.name)
        raw_gvas, _ = decompress_sav_to_gvas(data)
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        self._gvas_file = gvas_file
        self._get_file_size(data)
        self._set_active_data()
        return self

    def pal_count(self):
        return len(self._pals)

    def sav(self):
        logger.info("Converting %s to SAV", self.name)
        if (
            "Pal.PalWorldSaveGame" in self._gvas_file.header.save_game_class_name
            or "Pal.PalLocalWorldSaveGame"
            in self._gvas_file.header.save_game_class_name
        ):
            save_type = 0x32
        else:
            save_type = 0x31
        gvas = copy.deepcopy(self._gvas_file)
        return compress_gvas_to_sav(gvas.write(CUSTOM_PROPERTIES), save_type)

    def to_json_file(
        self,
        output_path,
        minify=False,
        allow_nan=True,
    ):
        logger.info("Converting %s to JSON, saving to %s", self.name, output_path)
        with open(output_path, "w", encoding="utf8") as f:
            indent = None if minify else "\t"
            json.dump(
                self._gvas_file.dump(),
                f,
                indent=indent,
                cls=CustomEncoder,
                allow_nan=allow_nan,
            )

    def to_sav_file(self, output_path):
        logger.info("Converting %s to SAV, saving to %s", self.name, output_path)
        if (
            "Pal.PalWorldSaveGame" in self._gvas_file.header.save_game_class_name
            or "Pal.PalLocalWorldSaveGame"
            in self._gvas_file.header.save_game_class_name
        ):
            save_type = 0x32
        else:
            save_type = 0x31

        logger.info("Compressing GVAS to SAV with save type %s", save_type)

        sav_file = compress_gvas_to_sav(
            self._gvas_file.write(CUSTOM_PROPERTIES), save_type
        )
        with open(output_path, "wb") as f:
            f.write(sav_file)

    async def update_pals(
        self, modified_pals: Dict[UUID, Pal], ws_callback=None
    ) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for pal_id, pal in modified_pals.items():
            await ws_callback(f"Updating pal {pal.nickname}")
            self._update_pal(pal_id, pal)

        logger.info("Updated %d pals in the save file.", len(modified_pals))

        await ws_callback("Saving changes to file")
        self._set_active_data()

    async def update_players(
        self, modified_players: Dict[UUID, Player], ws_callback=None
    ) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for uid, player in modified_players.items():
            await ws_callback(f"Updating player {player.nickname}")
            self._update_player(player)

        logger.info("Updated %d players in the save file.", len(modified_players))

        self._set_active_data()
        for uid, player in self._players.items():
            await ws_callback(f"Updating storage for player {player.nickname}")
            self._update_player_storage(uid)

    def _get_file_size(self, data: bytes):
        if hasattr(data, "seek") and hasattr(data, "tell"):
            data.seek(0, os.SEEK_END)
            self.size = data.tell()
            data.seek(0)
        else:
            self.size = data.__sizeof__()

    def _get_player_pals(self, uid):
        pals = {}
        pals = {k: v for k, v in self._pals.items() if f"{v.owner_uid}" == uid}
        return pals

    def _get_world_save_data(self, deep_copy=True):
        world_save_data = safe_get(self._gvas_file.properties, "worldSaveData", "value")
        if deep_copy:
            return copy.deepcopy(world_save_data)
        return world_save_data

    def _get_player_save_data(self, player_gvas: Dict[str, Any], deep_copy=True):
        player_save_data = safe_get(player_gvas.properties, "SaveData", "value")
        if deep_copy:
            return copy.deepcopy(player_save_data)
        return player_save_data

    def _is_player(self, entry):
        is_player_path = [
            "value",
            "RawData",
            "value",
            "object",
            "SaveParameter",
            "value",
            "IsPlayer",
            "value",
        ]
        return safe_get(entry, *is_player_path, default=False)

    def _load_pals(self):
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")
        self._pals = {}
        for e in self._character_save_parameter_map:
            if self._is_player(e):
                continue
            instance = Pal.create_summary(e)
            if instance:
                pal_summary = PalSummary(**instance.model_dump())
                self._pals[instance.instance_id] = pal_summary
            else:
                logger.warning("Failed to create PalEntity summary")

    def _set_active_data(self) -> None:
        world_save_data = self._get_world_save_data()
        self._character_save_parameter_map = safe_get(
            world_save_data, "CharacterSaveParameterMap", "value", default=[]
        )
        self._item_container_save_data = safe_get(
            world_save_data, "ItemContainerSaveData", "value", default=[]
        )
        self._dynamic_item_save_data = safe_get(
            world_save_data, "DynamicItemSaveData", "value", "values", default=[]
        )
        self._load_pals()
        self._get_players()

    def _get_players(self):
        if not self._character_save_parameter_map:
            return {}

        def extract_player_info(entry):
            uid = safe_get(entry, "key", "PlayerUId", "value")
            nickname_path = [
                "value",
                "RawData",
                "value",
                "object",
                "SaveParameter",
                "value",
                "NickName",
                "value",
            ]
            nickname = safe_get(entry, *nickname_path)
            level_path = [
                "value",
                "RawData",
                "value",
                "object",
                "SaveParameter",
                "value",
                "Level",
                "value",
            ]
            level = safe_get(entry, *level_path)
            player = Player(uid=uid.UUID(), nickname=nickname, level=level)
            player.pals = self._get_player_pals(uid)
            return player

        players = {
            x.uid: x
            for x in [
                extract_player_info(x)
                for x in self._character_save_parameter_map
                if self._is_player(x)
            ]
        }
        self._players = players

    def _get_player_uuid(self, uid: str):
        for player_id in self._players.keys():
            if uid.lower() == str(player_id).replace("-", "").lower():
                return player_id

    def _get_dynamic_item(self, local_id: UUID):
        item = None
        for entry in self._dynamic_item_save_data:
            current_local_id = safe_get(
                entry, "ID", "value", "LocalIdInCreatedWorld", "value"
            )
            if are_equal_uuids(current_local_id, local_id):
                item = entry
                break

        if item:
            item_type = safe_get(item, "RawData", "value", "type")
            durability = safe_get(item, "RawData", "value", "durability")
            remaining_bullets = safe_get(item, "RawData", "value", "remaining_bullets")
            item = DynamicItem(
                local_id=(
                    local_id.UUID() if isinstance(local_id, ArchiveUUID) else local_id
                ),
                durability=durability,
                remaining_bullets=remaining_bullets,
                type=item_type,
            )
        return item

    def _get_container_items(self, container: ItemContainer):
        container.slots = []
        for entry in self._item_container_save_data:
            current_container_id = safe_get(entry, "key", "ID", "value")
            if are_equal_uuids(current_container_id, container.id):
                slots = safe_get(entry, "value", "Slots", "value", "values", default=[])
                for slot in slots:
                    slot_index = safe_get(slot, "SlotIndex", "value")
                    static_id = safe_get(slot, "ItemId", "value", "StaticId", "value")
                    local_id = safe_get(
                        slot,
                        "ItemId",
                        "value",
                        "DynamicId",
                        "value",
                        "LocalIdInCreatedWorld",
                        "value",
                    )
                    dynamic_item = None
                    if str(local_id) != get_empty_property(PropertyType.UUID):
                        dynamic_item = self._get_dynamic_item(local_id)

                    count = safe_get(slot, "StackCount", "value")
                    container.slots.append(
                        ContainerSlot(
                            slot_index=slot_index,
                            static_id=static_id,
                            count=count,
                            dynamic_item=dynamic_item,
                        )
                    )
                break

    def _load_player_storage(self, uid: UUID, player_save_data: Dict[str, Any]):
        player = self._players[uid]
        inventory_info = safe_get(player_save_data, "InventoryInfo", "value")
        if not inventory_info:
            # Older save file had inventoryInfo 🤷‍♂️
            inventory_info = safe_get(
                player_save_data, "inventoryInfo", "value", default={}
            )

        if not inventory_info:
            logger.error("No inventory info found for player %s", uid)
            return

        common_container_id = safe_get(
            inventory_info, "CommonContainerId", "value", "ID", "value"
        )
        player.common_container = ItemContainer(
            id=common_container_id.UUID(), type="CommonContainer"
        )
        self._get_container_items(player.common_container)

        essential_container_id = safe_get(
            inventory_info, "EssentialContainerId", "value", "ID", "value"
        )
        player.essential_container = ItemContainer(
            id=essential_container_id.UUID(), type="EssentialContainer"
        )
        self._get_container_items(player.essential_container)

        weapon_load_out_container_id = safe_get(
            inventory_info, "WeaponLoadOutContainerId", "value", "ID", "value"
        )
        player.weapon_load_out_container = ItemContainer(
            id=weapon_load_out_container_id.UUID(), type="WeaponLoadOutContainer"
        )
        self._get_container_items(player.weapon_load_out_container)

        player_equipment_armor_container_id = safe_get(
            inventory_info, "PlayerEquipArmorContainerId", "value", "ID", "value"
        )
        player.player_equipment_armor_container = ItemContainer(
            id=player_equipment_armor_container_id.UUID(),
            type="PlayerEquipArmorContainer",
        )
        self._get_container_items(player.player_equipment_armor_container)

        food_equip_container_id = safe_get(
            inventory_info, "FoodEquipContainerId", "value", "ID", "value"
        )
        player.food_equip_container = ItemContainer(
            id=food_equip_container_id.UUID(), type="FoodEquipContainer"
        )
        self._get_container_items(player.food_equip_container)

    def _update_player_storage(self, uid: UUID):
        player_gvas_file = self._player_gvas_files.get(uid)
        if not player_gvas_file:
            logger.warning("No GvasFile found for player %s", uid)
            return
        player_save_data = self._get_player_save_data(player_gvas_file)
        self._load_player_storage(uid, player_save_data)

    def _update_pal(self, pal_id: UUID, pal: Pal) -> None:
        world_save_data = self._get_world_save_data(False)
        character_save_parameter_map = safe_get(
            world_save_data, "CharacterSaveParameterMap", "value", default=[]
        )
        for entry in character_save_parameter_map:
            current_instance_id = safe_get(entry, "key", "InstanceId", "value")
            if are_equal_uuids(current_instance_id, pal_id):
                self._update_pal_entry(entry, pal)
                return
        logger.warning("Pal with ID %s not found in the save file.", pal_id)

    def _update_pal_entry(self, entry: Dict[str, Any], pal: Pal) -> None:
        pal_obj = safe_get(
            entry, "value", "RawData", "value", "object", "SaveParameter", "value"
        )
        if not pal_obj:
            logger.error("Invalid pal entry structure for pal %s", pal.instance_id)
            return

        self._update_pal_nickname(pal_obj, pal.nickname)
        self._update_pal_gender(pal_obj, pal.gender)
        self._update_pal_equip_waza(pal_obj, pal.active_skills)
        self._update_mastered_waza(pal_obj, pal.learned_skills)
        self._update_pal_array(pal_obj, "PassiveSkillList", pal.passive_skills)

    def _update_pal_equip_waza(
        self, pal_obj: Dict[str, Any], active_skills: List[str]
    ) -> None:
        if not active_skills or len(active_skills) == 0:
            return
        active_skills = [f"EPalWazaID::{skill}" for skill in active_skills]
        if "EquipWaza" in pal_obj:
            safe_set(pal_obj["EquipWaza"], "value", "values", value=active_skills)
        else:
            pal_obj["EquipWaza"] = get_empty_property(PropertyType.ENUM_ARRAY)
            safe_set(pal_obj["EquipWaza"], "value", "values", value=active_skills)

    def _update_pal_gender(self, pal_obj: Dict[str, Any], gender: str) -> None:
        gender = f"EPalGenderType::{gender.capitalize()}"
        safe_set(pal_obj["Gender"], "value", "value", value=gender)

    def _update_pal_nickname(self, pal_obj: Dict[str, Any], nickname: str) -> None:
        if not nickname or len(nickname) == 0:
            return
        if "NickName" in pal_obj:
            safe_set(pal_obj["NickName"], "value", value=nickname)
        else:
            pal_obj["NickName"] = get_empty_property(PropertyType.STR)
            safe_set(pal_obj["NickName"], "value", value=nickname)

    def _update_mastered_waza(
        self, pal_obj: Dict[str, Any], learned_skills: List[str]
    ) -> None:
        if not learned_skills or len(learned_skills) == 0:
            return
        if "MasteredWaza" in pal_obj:
            safe_set(pal_obj["MasteredWaza"], "value", "values", value=learned_skills)
        else:
            pal_obj["MasteredWaza"] = get_empty_property(PropertyType.ENUM_ARRAY)
            safe_set(pal_obj["MasteredWaza"], "value", "values", value=learned_skills)

    def _update_pal_field(
        self, pal_obj: Dict[str, Any], field: str, value: Any
    ) -> None:
        if field in pal_obj:
            safe_set(pal_obj[field], "value", value=value)
        else:
            logger.warning("Field %s not found in pal object.", field)

    def _update_pal_array(
        self, pal_obj: Dict[str, Any], field: str, values: List[str]
    ) -> None:
        if field in pal_obj:
            safe_set(pal_obj[field], "value", "values", value=values)
        else:
            logger.warning("Array field %s not found in pal object.", field)

    def _update_player(self, player: Player) -> None:
        world_save_data = self._get_world_save_data(False)
        item_container_save_data = safe_get(
            world_save_data, "ItemContainerSaveData", "value", default=[]
        )
        dynamic_item_save_data = safe_get(
            world_save_data, "DynamicItemSaveData", "value", "values", default=[]
        )
        self._set_container_items(
            player.common_container, item_container_save_data, dynamic_item_save_data
        )
        self._set_container_items(
            player.essential_container, item_container_save_data, dynamic_item_save_data
        )
        self._set_container_items(
            player.weapon_load_out_container,
            item_container_save_data,
            dynamic_item_save_data,
        )
        self._set_container_items(
            player.player_equipment_armor_container,
            item_container_save_data,
            dynamic_item_save_data,
        )
        self._set_container_items(
            player.food_equip_container,
            item_container_save_data,
            dynamic_item_save_data,
        )

    def _set_dynamic_data(
        self,
        static_id: str,
        dynamic_item: DynamicItem,
        dynamic_item_data: Dict[str, Any],
    ) -> Dict[str, Any]:
        # Set UUIDs
        safe_set(
            dynamic_item_data,
            "ID",
            "value",
            "LocalIdInCreatedWorld",
            "value",
            value=str(dynamic_item.local_id),
        )
        safe_set(
            dynamic_item_data,
            "RawData",
            "value",
            "id",
            "local_id_in_created_world",
            value=str(dynamic_item.local_id),
        )
        # Set static ID
        safe_set(dynamic_item_data, "StaticItemId", "value", value=static_id)
        safe_set(
            dynamic_item_data, "RawData", "value", "id", "static_id", value=static_id
        )
        safe_set(dynamic_item_data, "RawData", "value", "type", value=dynamic_item.type)
        if dynamic_item.type == "armor":
            safe_set(
                dynamic_item_data,
                "RawData",
                "value",
                "durability",
                value=dynamic_item.durability,
            )
            safe_remove(dynamic_item_data, "RawData", "value", "remaining_bullets")
            safe_remove(dynamic_item_data, "RawData", "value", "passive_skill_list")

        if dynamic_item.type == "weapon":
            safe_set(
                dynamic_item_data,
                "RawData",
                "value",
                "durability",
                value=dynamic_item.durability,
            )
            safe_set(
                dynamic_item_data,
                "RawData",
                "value",
                "remaining_bullets",
                value=dynamic_item.remaining_bullets,
            )
            passive_skill_list = safe_get(
                dynamic_item_data, "RawData", "value", "passive_skill_list", None
            )
            if not passive_skill_list:
                safe_set(
                    dynamic_item_data,
                    "RawData",
                    "value",
                    "passive_skill_list",
                    value=[],
                )

    def _set_dynamic_item(
        self,
        slot: Dict[str, Any],
        container_slot: ContainerSlot,
        dynamic_item_save_data: List[Dict[str, Any]],
    ):
        slot_local_id = safe_get(
            slot,
            "ItemId",
            "value",
            "DynamicId",
            "value",
            "LocalIdInCreatedWorld",
            "value",
        )
        # New container slot does not have a dynamic item, we need to check if slot
        # has a dynamic item, if it does we need to delete it
        if (
            not container_slot.dynamic_item
            and not is_empty_uuid(slot_local_id)
            and is_valid_uuid(str(slot_local_id))
        ):
            for entry in dynamic_item_save_data:
                local_id = safe_get(
                    entry, "ID", "value", "LocalIdInCreatedWorld", "value"
                )
                if are_equal_uuids(local_id, slot_local_id):
                    dynamic_item_save_data.remove(entry)
                    break
            return get_empty_property(PropertyType.UUID)

        if not container_slot.dynamic_item:
            return get_empty_property(PropertyType.UUID)

        if is_empty_uuid(container_slot.dynamic_item.local_id) and is_empty_uuid(
            slot_local_id
        ):
            container_slot.dynamic_item.local_id = uuid.uuid4()
            new_dynamic_item = get_empty_property(PropertyType.DYNAMIC_ITEM)
            self._set_dynamic_data(
                container_slot.static_id,
                container_slot.dynamic_item,
                new_dynamic_item,
            )
            dynamic_item_save_data.append(new_dynamic_item)
            return container_slot.dynamic_item.local_id

        if is_empty_uuid(container_slot.dynamic_item.local_id) and not is_empty_uuid(
            slot_local_id
        ):
            container_slot.dynamic_item.local_id = slot_local_id

        # If the dynamic item is not empty, we need to update it
        for entry in dynamic_item_save_data:
            local_id = safe_get(entry, "ID", "value", "LocalIdInCreatedWorld", "value")
            if are_equal_uuids(local_id, container_slot.dynamic_item.local_id):
                self._set_dynamic_data(
                    container_slot.static_id, container_slot.dynamic_item, entry
                )
                return container_slot.dynamic_item.local_id

    def _set_container_items(
        self,
        container: ItemContainer,
        item_container_save_data: List[Dict[str, Any]],
        dynamic_item_save_data: List[Dict[str, Any]],
    ):
        for entry in item_container_save_data:
            current_container_id = safe_get(entry, "key", "ID", "value")
            if not are_equal_uuids(current_container_id, container.id):
                continue
            slots = safe_get(entry, "value", "Slots", "value", "values", default=[])
            for slot in slots:
                slot_index = safe_get(slot, "SlotIndex", "value")
                container_slot = container.get_slot(slot_index)
                local_id = self._set_dynamic_item(
                    slot,
                    container_slot,
                    dynamic_item_save_data,
                )
                safe_set(
                    slot,
                    "ItemId",
                    "value",
                    "DynamicId",
                    "value",
                    "LocalIdInCreatedWorld",
                    "value",
                    value=str(local_id),
                )
                safe_set(
                    slot,
                    "ItemId",
                    "value",
                    "StaticId",
                    "value",
                    value=container_slot.static_id,
                )
                safe_set(slot, "StackCount", "value", value=container_slot.count)
            break
