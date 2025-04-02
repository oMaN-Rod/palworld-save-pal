from enum import Enum
from typing import Any, Dict, List, Optional
from uuid import UUID

from palworld_save_tools.archive import UUID as ArchiveUUID

from palworld_save_pal.game.item_container_slot import (
    ItemContainerSlot as IContainerSlot,
)
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class PrefixedEnum(Enum):
    @classmethod
    def _prefix(cls):
        return getattr(cls, "_enum_prefix", f"{cls.__name__}::")

    def prefixed(self):
        return f"{self._enum_prefix.value}{self.value}"


class ArrayType(str, Enum):
    BYTE_PROPERTY = "ByteProperty"
    ENUM_PROPERTY = "EnumProperty"
    NAME_PROPERTY = "NameProperty"
    STRUCT_PROPERTY = "StructProperty"


class Element(str, Enum):
    """Element types in the game"""

    NEUTRAL = "Normal"
    DARK = "Dark"
    DRAGON = "Dragon"
    ICE = "Ice"
    FIRE = "Fire"
    GRASS = "Leaf"
    GROUND = "Earth"
    ELECTRIC = "Electricity"
    WATER = "Water"
    UNKNOWN = "Unknown"

    @classmethod
    def get_all_elements(cls) -> list[str]:
        """Get all element type values"""
        return [member.value for member in cls]

    @classmethod
    def from_value(cls, value: str) -> "Element":
        """Convert from game's enum format to our enum"""
        type_str = value.split("::")[-1]
        try:
            return next((t for t in cls if t.value == type_str), cls.UNKNOWN)
        except KeyError:
            return cls.UNKNOWN


class EntryState(str, Enum):
    NONE = "None"
    MODIFIED = "Modified"
    NEW = "New"
    DELETED = "Deleted"


class GroupType(str, PrefixedEnum):
    _enum_prefix = "EPalGroupType::"

    GUILD = "Guild"
    ORGANIZATION = "Organization"

    @staticmethod
    def from_value(value: str):
        try:
            value = value.replace(GroupType._enum_prefix.value, "")
            return GroupType(value)
        except:
            logger.warning("%s is not a valid group type", value)


class PalGender(str, PrefixedEnum):
    _enum_prefix = "EPalGenderType::"

    MALE = "Male"
    FEMALE = "Female"

    @staticmethod
    def from_value(value: str):
        try:
            value = value.replace(PalGender._enum_prefix.value, "")
            return PalGender(value)
        except:
            logger.warning("%s is not a valid gender, defaulting to female", value)
            return PalGender.FEMALE


class PalRank(int, Enum):
    RANK0 = 1
    RANK1 = 2
    RANK2 = 3
    RANK3 = 4
    RANK4 = 5

    def get_index(self):
        return self.value - 1

    @staticmethod
    def from_value(value: int):
        try:
            return PalRank(value)
        except:
            logger.warning("%s is not a valid rank", value)


class WorkSuitability(str, PrefixedEnum):
    _enum_prefix = "EPalWorkSuitability::"

    EMIT_FLAME = "EmitFlame"
    WATERING = "Watering"
    SEEDING = "Seeding"
    GENERATE_ELECTRICITY = "GenerateElectricity"
    HANDCRAFT = "Handcraft"
    COLLECTION = "Collection"
    DEFOREST = "Deforest"
    MINING = "Mining"
    OIL_EXTRACTION = "OilExtraction"
    PRODUCT_MEDICINE = "ProductMedicine"
    COOL = "Cool"
    TRANSPORT = "Transport"
    MONSTER_FARM = "MonsterFarm"

    @staticmethod
    def from_value(value: str):
        try:
            value = value.replace(WorkSuitability._enum_prefix.value, "")
            return WorkSuitability(value)
        except:
            logger.warning("%s is not a valid work suitability", value)


def toUUID(guid: Any) -> Optional[UUID]:
    if isinstance(guid, UUID):
        return guid
    if isinstance(guid, str):
        return UUID(guid)
    if isinstance(guid, ArchiveUUID):
        return guid.UUID()


class PalObjects:
    EMPTY_UUID = toUUID("00000000-0000-0000-0000-000000000000")
    TIME = 638486453957560000

    @staticmethod
    def StrProperty(value: str):
        return {
            "id": None,
            "type": "StrProperty",
            "value": value,
        }

    @staticmethod
    def NameProperty(value: str):
        return {
            "id": None,
            "value": value,
            "type": "NameProperty",
        }

    @staticmethod
    def BoolProperty(value: bool):
        return {
            "id": None,
            "type": "BoolProperty",
            "value": value,
        }

    @staticmethod
    def IntProperty(value: int):
        return {
            "id": None,
            "type": "IntProperty",
            "value": value,
        }

    @staticmethod
    def Int64Property(value: int):
        return {
            "id": None,
            "type": "Int64Property",
            "value": value,
        }

    @staticmethod
    def FloatProperty(value: float):
        return {
            "id": None,
            "type": "FloatProperty",
            "value": value,
        }

    @staticmethod
    def Guid(value: str | UUID):
        return {
            "struct_type": "Guid",
            "struct_id": PalObjects.EMPTY_UUID,
            "id": None,
            "value": toUUID(value),
            "type": "StructProperty",
        }

    @staticmethod
    def get_guid(d: Dict[str, Any]) -> Optional[UUID]:
        value = PalObjects.get_value(d)
        return toUUID(value)

    @staticmethod
    def as_uuid(value: Any) -> Optional[UUID]:
        return toUUID(value)

    @staticmethod
    def set_value(d: Dict[str, Any], value: Any):
        d["value"] = value

    @staticmethod
    def get_value(d: Dict[str, Any], default: Any = None) -> Optional[Any]:
        return PalObjects.get_nested(d, "value", default=default)

    @staticmethod
    def get_nested(d: Dict[str, Any], *keys: str, default: Any = None) -> Any:
        try:
            return (
                d[keys[0]]
                if len(keys) == 1
                else PalObjects.get_nested(d[keys[0]], *keys[1:], default=default)
            )
        except (KeyError, TypeError, IndexError):
            logger.warning("Key(s) not found: %s", keys)
            return default

    @staticmethod
    def set_nested(d: dict, *keys: str, value: Any) -> None:
        for key in keys[:-1]:
            if key not in d:
                raise KeyError(f"Key not found: {key}, {keys}, {d.keys()}")
            d = d[key]
        d[keys[-1]] = value

    @staticmethod
    def EnumProperty(type: str, value: str):
        return {
            "id": None,
            "type": "EnumProperty",
            "value": {"type": type, "value": value},
        }

    @staticmethod
    def ByteProperty(value: int):
        return {
            "id": None,
            "value": {
                "type": "None",
                "value": value,
            },
            "type": "ByteProperty",
        }

    @staticmethod
    def get_byte_property(d: Dict[str, Any]) -> Optional[str]:
        return PalObjects.get_nested(d, "value", "value")

    @staticmethod
    def set_byte_property(d: Dict[str, Any], value: str):
        PalObjects.set_nested(d, "value", "value", value=value)

    @staticmethod
    def get_enum_property(d: Dict[str, Any]) -> Optional[str]:
        return PalObjects.get_nested(d, "value", "value")

    @staticmethod
    def set_enum_property(d: Dict[str, Any], value: str):
        PalObjects.set_nested(d, "value", "value", value=value)

    @staticmethod
    def ArrayPropertyValues(
        array_type: ArrayType, values: List[Any], custom_type: Optional[str] = None
    ):
        struct = {
            "array_type": array_type.value,
            "id": None,
            "value": {"values": values},
            "type": "ArrayProperty",
        }

        if custom_type:
            struct["custom_type"] = custom_type
        return struct

    @staticmethod
    def ArrayProperty(
        array_type: ArrayType,
        value: Optional[Dict[str, Any]] = None,
        custom_type: Optional[str] = None,
    ):
        struct = {
            "array_type": array_type.value,
            "id": None,
            "value": value,
            "type": "ArrayProperty",
        }

        if custom_type:
            struct["custom_type"] = custom_type

        return struct

    @staticmethod
    def get_array_property(d: Dict[str, Any]) -> Optional[List[Any]]:
        return PalObjects.get_nested(d, "value", "values", default=[])

    @staticmethod
    def append_array_item(d: Dict[str, Any], value: Any):
        PalObjects.get_array_property(d).append(value)

    @staticmethod
    def pop_array_item(d: Dict[str, Any], index: Any) -> Optional[Any]:
        return PalObjects.get_array_property(d).pop(index)

    @staticmethod
    def set_array_property(d: Dict[str, Any], values: List[Any]):
        PalObjects.set_nested(d, "value", "values", value=values)

    @staticmethod
    def FixedPoint64(value: int):
        return {
            "struct_type": "FixedPoint64",
            "struct_id": PalObjects.EMPTY_UUID,
            "id": None,
            "value": {"Value": PalObjects.Int64Property(value)},
            "type": "StructProperty",
        }

    @staticmethod
    def get_fixed_point64(d: Dict[str, Any]) -> Optional[int]:
        return PalObjects.get_value(d["value"]["Value"])

    @staticmethod
    def set_fixed_point64(d: Dict[str, Any], value: int):
        PalObjects.set_value(d["value"]["Value"], value)

    @staticmethod
    def PalContainerId(id: UUID | str):
        return {
            "struct_type": "PalContainerId",
            "struct_id": PalObjects.EMPTY_UUID,
            "id": None,
            "value": {"ID": PalObjects.Guid(id)},
            "type": "StructProperty",
        }

    @staticmethod
    def get_pal_container_id(d: Dict[str, Any]) -> Optional[UUID]:
        return PalObjects.get_value(d["value"]["ID"])

    @staticmethod
    def set_pal_container_id(d: Dict[str, Any], container_id: str | UUID):
        container_id = toUUID(container_id)
        PalObjects.set_value(d["value"]["ID"], container_id)

    @staticmethod
    def PalCharacterSlotId(container_id: UUID | str, slot_idx: int):
        return {
            "struct_type": "PalCharacterSlotId",
            "struct_id": PalObjects.EMPTY_UUID,
            "id": None,
            "value": {
                "ContainerId": PalObjects.PalContainerId(container_id),
                "SlotIndex": PalObjects.IntProperty(slot_idx),
            },
            "type": "StructProperty",
        }

    @staticmethod
    def get_pal_character_slot_id(d: Dict[str, Any]) -> Optional[tuple[UUID, int]]:
        container_id = PalObjects.get_pal_container_id(
            PalObjects.get_nested(d, "value", "ContainerId")
        )
        slot_idx = PalObjects.get_value(d["value"]["SlotIndex"])

        if container_id is None or slot_idx is None:
            return
        return (container_id, slot_idx)

    @staticmethod
    def set_pal_character_slot_id(d: Dict[str, Any], container_id: UUID, slot_idx: int):
        PalObjects.set_pal_container_id(d["value"]["ContainerId"], container_id)
        PalObjects.set_value(d["value"]["SlotIndex"], slot_idx)

    @staticmethod
    def individual_character_handle_ids(instance_id: UUID, guid: UUID | None = None):
        if not guid:
            guid = PalObjects.EMPTY_UUID
        return {
            "guid": guid,
            "instance_id": instance_id,
        }

    @staticmethod
    def FloatContainer(d: Dict[str, Any] = None):
        return {
            "struct_type": "FloatContainer",
            "struct_id": PalObjects.EMPTY_UUID,
            "id": None,
            "value": d or {},
            "type": "StructProperty",
        }

    @staticmethod
    def ContainerSlotData(slot_idx: int, instance_id: UUID):
        return {
            "SlotIndex": PalObjects.IntProperty(slot_idx),
            "RawData": PalObjects.ArrayProperty(
                ArrayType.BYTE_PROPERTY,
                {
                    "player_uid": PalObjects.EMPTY_UUID,
                    "instance_id": instance_id,
                    "permission_tribe_id": 0,
                },
                custom_type=".worldSaveData.CharacterContainerSaveData.Value.Slots.Slots.RawData",
            ),
        }

    @staticmethod
    def DateTime(time):
        return {
            "struct_type": "DateTime",
            "struct_id": PalObjects.EMPTY_UUID,
            "id": None,
            "value": time,
            "type": "StructProperty",
        }

    @staticmethod
    def Vector(x, y, z):
        return {
            "struct_type": "Vector",
            "struct_id": PalObjects.EMPTY_UUID,
            "id": None,
            "value": {
                "x": x,
                "y": y,
                "z": z,
            },
            "type": "StructProperty",
        }

    @staticmethod
    def PalLoggedinPlayerSaveDataRecordData(d: Dict[str, Any] = None):
        return {
            "struct_type": "PalLoggedinPlayerSaveDataRecordData",
            "struct_id": PalObjects.EMPTY_UUID,
            "id": None,
            "value": d or {},
            "type": "StructProperty",
        }

    @staticmethod
    def MapProperty(
        key_type: str, value_type: str, key_struct_type=None, value_struct_type=None
    ):
        return {
            "key_type": key_type,
            "value_type": value_type,
            "key_struct_type": key_struct_type,
            "value_struct_type": value_struct_type,
            "id": None,
            "value": [],
            "type": "MapProperty",
        }

    @staticmethod
    def get_map_property(d: dict) -> Optional[list[dict]]:
        return PalObjects.get_value(d)

    @staticmethod
    def WorkSuitabilityStruct(work_suitability: str, rank: int):
        return {
            "WorkSuitability": PalObjects.EnumProperty(
                "EPalWorkSuitability", work_suitability
            ),
            "Rank": PalObjects.IntProperty(rank),
        }

    @staticmethod
    def StatusPointStruct(name: str, point: int):
        return {
            "StatusName": PalObjects.NameProperty(name),
            "StatusPoint": PalObjects.IntProperty(point),
        }

    StatusNames = [
        "最大HP",  # Max HP
        "最大SP",  # Max SP
        "攻撃力",  # Attack Power
        "所持重量",  # Carrying Capacity
        "捕獲率",  # Capture Rate
        "作業速度",  # Work Speed
    ]

    StatusNameMap = {
        "最大HP": "max_hp",
        "最大SP": "max_sp",
        "攻撃力": "attack",
        "所持重量": "weight",
        "捕獲率": "capture_rate",
        "作業速度": "work_speed",
    }

    ExStatusNames = [
        "最大HP",  # Max HP
        "最大SP",  # Max SP
        "攻撃力",  # Attack Power
        "所持重量",  # Carrying Capacity
        "作業速度",  # Work Speed
    ]

    ExStatusNameMap = {
        "最大HP": "max_hp",
        "最大SP": "max_sp",
        "攻撃力": "attack",
        "所持重量": "weight",
        "作業速度": "work_speed",
    }

    @staticmethod
    def GetStatusPointList(
        prop_name: str, status_points: Dict[str, str]
    ) -> Dict[str, Any]:
        return PalObjects.ArrayProperty(
            ArrayType.STRUCT_PROPERTY,
            {
                "prop_name": prop_name,
                "prop_type": "StructProperty",
                "values": [
                    PalObjects.StatusPointStruct(name, 0) for name in status_points
                ],
                "type_name": "PalGotStatusPoint",
                "id": PalObjects.EMPTY_UUID,
            },
        )

    @staticmethod
    def PalSaveParameter(
        character_id: str,
        instance_id: UUID | str,
        owner_uid: UUID | str,
        container_id: UUID | str,
        slot_idx: int,
        group_id: UUID | str,
        nickname: Optional[str] = None,
        active_skills: List[str] = None,
        passive_skills: List[str] = None,
        work_suitability_data: Dict[str, int] = None,
    ):
        nickname = nickname or character_id
        active_skills = active_skills or []
        passive_skills = passive_skills or []

        work_suitability = []
        if work_suitability_data:
            for work, value in work_suitability_data.items():
                suitability = WorkSuitability.from_value(work)
                work_suitability.append(
                    PalObjects.WorkSuitabilityStruct(suitability.prefixed(), value)
                )
        else:
            work_suitability = [
                PalObjects.WorkSuitabilityStruct(work.prefixed(), 0)
                for work in WorkSuitability
            ]

        return {
            "key": {
                "PlayerUId": PalObjects.Guid(PalObjects.EMPTY_UUID),
                "InstanceId": PalObjects.Guid(instance_id),
                "DebugName": PalObjects.StrProperty(""),
            },
            "value": {
                "RawData": PalObjects.ArrayProperty(
                    ArrayType.BYTE_PROPERTY,
                    {
                        "object": {
                            "SaveParameter": {
                                "struct_type": "PalIndividualCharacterSaveParameter",
                                "struct_id": PalObjects.EMPTY_UUID,
                                "id": None,
                                "value": {
                                    "CharacterID": PalObjects.NameProperty(
                                        character_id
                                    ),
                                    "Gender": PalObjects.EnumProperty(
                                        "EPalGenderType", PalGender.FEMALE.prefixed()
                                    ),
                                    "Level": PalObjects.ByteProperty(1),
                                    "Exp": PalObjects.Int64Property(0),
                                    "NickName": PalObjects.StrProperty(nickname),
                                    "EquipWaza": PalObjects.ArrayPropertyValues(
                                        ArrayType.ENUM_PROPERTY, active_skills
                                    ),
                                    "MasteredWaza": PalObjects.ArrayPropertyValues(
                                        ArrayType.ENUM_PROPERTY, []
                                    ),
                                    "HP": PalObjects.FixedPoint64(545000),
                                    "Talent_HP": PalObjects.ByteProperty(50),
                                    "Talent_Shot": PalObjects.ByteProperty(50),
                                    "Talent_Defense": PalObjects.ByteProperty(50),
                                    "FullStomach": PalObjects.FloatProperty(300),
                                    "PassiveSkillList": PalObjects.ArrayPropertyValues(
                                        ArrayType.NAME_PROPERTY, passive_skills
                                    ),
                                    "OwnedTime": PalObjects.DateTime(PalObjects.TIME),
                                    "OwnerPlayerUId": PalObjects.Guid(owner_uid),
                                    "OldOwnerPlayerUIds": PalObjects.ArrayProperty(
                                        ArrayType.STRUCT_PROPERTY,
                                        {
                                            "prop_name": "OldOwnerPlayerUIds",
                                            "prop_type": "StructProperty",
                                            "values": [owner_uid],
                                            "type_name": "Guid",
                                            "id": PalObjects.EMPTY_UUID,
                                        },
                                    ),
                                    "SlotID": PalObjects.PalCharacterSlotId(
                                        container_id, slot_idx
                                    ),
                                    "GotStatusPointList": PalObjects.GetStatusPointList(
                                        "GotStatusPointList", PalObjects.StatusNames
                                    ),
                                    "GotExStatusPointList": PalObjects.GetStatusPointList(
                                        "GotExStatusPointList", PalObjects.ExStatusNames
                                    ),
                                    "LastJumpedLocation": PalObjects.Vector(
                                        0, 0, 7088.5
                                    ),
                                },
                                "type": "StructProperty",
                            }
                        },
                        "unknown_bytes": [0, 0, 0, 0],
                        "group_id": group_id,
                    },
                    custom_type=".worldSaveData.CharacterSaveParameterMap.Value.RawData",
                )
            },
            "CustomVersionData": {
                "array_type": "ByteProperty",
                "id": None,
                "value": {
                    "values": [
                        1,
                        0,
                        0,
                        0,
                        108,
                        246,
                        252,
                        15,
                        153,
                        72,
                        144,
                        17,
                        248,
                        156,
                        96,
                        177,
                        94,
                        71,
                        70,
                        74,
                        1,
                        0,
                        0,
                        0,
                    ]
                },
                "type": "ArrayProperty",
            },
        }

    @staticmethod
    def DynamicItem(container_slot: IContainerSlot):
        return {
            "RawData": PalObjects.ArrayProperty(
                ArrayType.BYTE_PROPERTY,
                {
                    "id": {
                        "created_world_id": PalObjects.EMPTY_UUID,
                        "local_id_in_created_world": container_slot.dynamic_item.local_id,
                        "static_id": container_slot.static_id,
                    },
                    "type": container_slot.dynamic_item.type,
                    "durability": container_slot.dynamic_item.durability,
                },
                custom_type=".worldSaveData.DynamicItemSaveData.DynamicItemSaveData.RawData",
            ),
            "CustomVersionData": PalObjects.ArrayPropertyValues(
                ArrayType.BYTE_PROPERTY, [0, 0, 0, 0]
            ),
        }

    @staticmethod
    def ItemContainerSlot(container_slot: IContainerSlot) -> Dict[str, Any]:
        return {
            "RawData": PalObjects.ArrayProperty(
                ArrayType.BYTE_PROPERTY,
                {
                    "slot_index": container_slot.slot_index,
                    "count": container_slot.count,
                    "item": {
                        "static_id": container_slot.static_id,
                        "dynamic_id": {
                            "created_world_id": PalObjects.EMPTY_UUID,
                            "local_id_in_created_world": (
                                PalObjects.EMPTY_UUID
                                if not container_slot.dynamic_item
                                else container_slot.dynamic_item.local_id
                            ),
                            "static_id": container_slot.static_id,
                        },
                    },
                    "trailing_bytes": [0] * 16,
                },
                custom_type=".worldSaveData.ItemContainerSaveData.Value.Slots.Slots.RawData",
            ),
            "CustomVersionData": {
                "array_type": "ByteProperty",
                "id": None,
                "value": {
                    "values": [
                        1,
                        0,
                        0,
                        0,
                        126,
                        180,
                        234,
                        18,
                        154,
                        27,
                        90,
                        255,
                        113,
                        170,
                        113,
                        188,
                        223,
                        51,
                        214,
                        14,
                        1,
                        0,
                        0,
                        0,
                    ]
                },
                "type": "ArrayProperty",
            },
        }

    @staticmethod
    def GotWorkSuitabilityRankList(work_suitabilities: Dict[WorkSuitability, int]):
        return PalObjects.ArrayProperty(
            ArrayType.STRUCT_PROPERTY,
            {
                "prop_name": "GotWorkSuitabilityAddRankList",
                "prop_type": "StructProperty",
                "values": [
                    PalObjects.WorkSuitabilityStruct(work.prefixed(), rank)
                    for work, rank in work_suitabilities.items()
                ],
                "type_name": "PalWorkSuitabilityInfo",
                "id": "00000000-0000-0000-0000-000000000000",
            },
        )
