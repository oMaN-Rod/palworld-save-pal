from enum import Enum
from typing import Any, Dict, List, Optional
from uuid import UUID

from palworld_save_tools.archive import UUID as ArchiveUUID

from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

class PrefixedEnum(Enum):
    @classmethod
    def _prefix(cls):
        return getattr(cls, '_enum_prefix', f"{cls.__name__}::")

    def prefixed(self):
        return f"{self._prefix()}{self.value}"

class ArrayType(str, Enum):
    BYTE_PROPERTY = "ByteProperty"
    ENUM_PROPERTY = "EnumProperty"
    NAME_PROPERTY = "NameProperty"

class Element(str, Enum):
    FIRE = "Fire"
    WATER = "Water"
    GROUND = "Ground"
    ICE = "Ice"
    NEUTRAL = "Neutral"
    DARK = "Dark"
    GRASS = "Grass"
    DRAGON = "Dragon"
    ELECTRIC = "Electric"
    
    @staticmethod
    def from_value(value: str):
        try:
            return Element(value)
        except:
            logger.warning("%s is not a valid element", value)

class PalGender(str, PrefixedEnum):
    _enum_prefix = "EPalGenderType::"
    
    MALE = "Male"
    FEMALE = "Female"
    
    @staticmethod
    def from_value(value: str):
        try:
            value = value.replace("EPalGenderType::", "")
            return PalGender(value)
        except:
            logger.warning("%s is not a valid gender", value)
            
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
            value = value.replace("EPalWorkSuitability::", "")
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
            "type": "NameProperty",
            "value": value,
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
        logger.debug("Getting GUID from %s", d)
        value = PalObjects.get_value(d)
        logger.debug("GUID value: %s", value)
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
    def get_enum_property(d: Dict[str, Any]) -> Optional[str]:
        return PalObjects.get_nested(d, "value", "value")
    
    @staticmethod
    def set_enum_property(d: Dict[str, Any], value: str):
        PalObjects.set_nested(d, "value", "value", value)
        
    @staticmethod
    def ArrayProperty(array_type: ArrayType, values: List[str], custom_type: Optional[str] = None):
        struct = {
            "array_type": array_type.value(),
            "id": None,
            "value": {
                "values": values
            },
            "type": "ArrayProperty",
        }

        if custom_type:
            struct["custom_type"] = custom_type

        return struct
    
    @staticmethod
    def get_array_property(d: Dict[str, Any]) -> Optional[List[Any]]:
        return PalObjects.get_nested(d, "value", "values")
    
    @staticmethod
    def append_array_item(d: Dict[str, Any], value: Any):
        PalObjects.get_array_property(d).append(value)
        
    @staticmethod
    def pop_array_item(d: Dict[str, Any], index: Any) -> Optional[Any]:
        return PalObjects.get_array_property(d).pop(index)
    
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
            return None
        return (container_id, slot_idx)
    
    @staticmethod
    def set_pal_character_slot_id(d: Dict[str, Any], container_id: UUID | str, slot_idx: int):
        PalObjects.set_pal_container_id(d["value"]["ContainerId"], container_id)
        PalObjects.set_value(d["value"]["SlotIndex"], slot_idx)
        
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
    def ContainerSlotData(slot_idx: int):
        return {
            "SlotIndex": PalObjects.IntProperty(slot_idx),
            "RawData": PalObjects.ArrayProperty(ArrayType.BYTE_PROPERTY, {
                    "player_uid": PalObjects.EMPTY_UUID,
                    "instance_id": PalObjects.EMPTY_UUID,
                    "permission_tribe_id": 0,
                }, '.worldSaveData.CharacterContainerSaveData.Value.Slots.Slots.RawData')
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
        "最大HP", # Max HP
        "最大SP", # Max SP
        "攻撃力", # Attack Power
        "所持重量", # Carrying Capacity
        "捕獲率", # Capture Rate
        "作業速度", # Work Speed
    ]

    ExStatusNames = [
        "最大HP", # Max HP
        "最大SP", # Max SP
        "攻撃力", # Attack Power
        "所持重量", # Carrying Capacity
        "作業速度", # Work Speed
    ]
        
    @staticmethod
    def PalSaveParameter(instance_id: UUID | str, owner_uid: UUID | str, container_id: UUID | str, slot_idx: int, group_id: UUID | str):
        return {
            "key": {
                "PlayerUId": PalObjects.Guid(PalObjects.EMPTY_UUID),
                "InstanceId": PalObjects.Guid(instance_id),
                "DebugName": PalObjects.StrProperty(""),
            },
            "value": {
                "RawData": PalObjects.ArrayProperty(
                    "ByteProperty",
                    {
                        "object": {
                            "SaveParameter": {
                                "struct_type": "PalIndividualCharacterSaveParameter",
                                "struct_id": PalObjects.EMPTY_UUID,
                                "id": None,
                                "value": {
                                    "CharacterID": PalObjects.NameProperty("SheepBall"),
                                    "Gender": PalObjects.EnumProperty(
                                        "EPalGenderType", "EPalGenderType::Female"
                                    ),
                                    "Level": PalObjects.IntProperty(1),
                                    "Exp": PalObjects.IntProperty(0),
                                    "NickName": PalObjects.StrProperty("!!!NEW PAL!!!"),
                                    "EquipWaza": PalObjects.ArrayProperty(
                                        "EnumProperty", {"values": []}
                                    ),
                                    "MasteredWaza": PalObjects.ArrayProperty(
                                        "EnumProperty", {"values": []}
                                    ),
                                    "HP": PalObjects.FixedPoint64(545000),
                                    "Talent_HP": PalObjects.IntProperty(50),
                                    "Talent_Melee": PalObjects.IntProperty(50),
                                    "Talent_Shot": PalObjects.IntProperty(50),
                                    "Talent_Defense": PalObjects.IntProperty(50),
                                    "FullStomach": PalObjects.FloatProperty(300),
                                    "PassiveSkillList": PalObjects.ArrayProperty(
                                        "NameProperty", {"values": []}
                                    ),
                                    "MP": PalObjects.FixedPoint64(10000),
                                    "OwnedTime": PalObjects.DateTime(PalObjects.TIME),
                                    "OwnerPlayerUId": PalObjects.Guid(owner_uid),
                                    "OldOwnerPlayerUIds": PalObjects.ArrayProperty(
                                        "StructProperty",
                                        {
                                            "prop_name": "OldOwnerPlayerUIds",
                                            "prop_type": "StructProperty",
                                            "values": [owner_uid],
                                            "type_name": "Guid",
                                            "id": PalObjects.EMPTY_UUID,
                                        },
                                    ),
                                    # MaxHP is no longer stored in the game save.
                                    # "MaxHP": PalObjects.FixedPoint64(545000),
                                    "CraftSpeed": PalObjects.IntProperty(70),
                                    # Do not omit CraftSpeeds, otherwise the pal works super slow
                                    # TODO use accurate data (even tho this is useless)
                                    "CraftSpeeds": PalObjects.ArrayProperty(
                                        "StructProperty",
                                        {
                                            "prop_name": "CraftSpeeds",
                                            "prop_type": "StructProperty",
                                            "values": [
                                                PalObjects.WorkSuitabilityStruct(
                                                    work.prefixed(), 0
                                                )
                                                for work in WorkSuitability
                                            ],
                                            "type_name": "PalWorkSuitabilityInfo",
                                            "id": PalObjects.EMPTY_UUID,
                                        },
                                    ),
                                    "SanityValue": PalObjects.FloatProperty(100.0),
                                    "EquipItemContainerId": PalObjects.PalContainerId(
                                        str(uuid.uuid4())
                                    ),
                                    "SlotID": PalObjects.PalCharacterSlotId(
                                        slot_idx, container_id
                                    ),
                                    # TODO Need accurate values
                                    "MaxFullStomach": PalObjects.FloatProperty(300.0),
                                    "GotStatusPointList": PalObjects.ArrayProperty(
                                        "StructProperty",
                                        {
                                            "prop_name": "GotStatusPointList",
                                            "prop_type": "StructProperty",
                                            "values": [
                                                PalObjects.StatusPointStruct(name, 0)
                                                for name in PalObjects.StatusNames
                                            ],
                                            "type_name": "PalGotStatusPoint",
                                            "id": PalObjects.EMPTY_UUID,
                                        },
                                    ),
                                    "GotExStatusPointList": PalObjects.ArrayProperty(
                                        "StructProperty",
                                        {
                                            "prop_name": "GotExStatusPointList",
                                            "prop_type": "StructProperty",
                                            "values": [
                                                PalObjects.StatusPointStruct(name, 0)
                                                for name in PalObjects.ExStatusNames
                                            ],
                                            "type_name": "PalGotStatusPoint",
                                            "id": PalObjects.EMPTY_UUID,
                                        },
                                    ),
                                    "DecreaseFullStomachRates": PalObjects.FloatContainer(),
                                    "CraftSpeedRates": PalObjects.FloatContainer(),
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
                    ".worldSaveData.CharacterSaveParameterMap.Value.RawData",
                )
            },
        }