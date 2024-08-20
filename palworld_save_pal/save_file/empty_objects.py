import copy
from enum import Enum
from typing import Dict, Any

EMPTY_STR_PROPERTY: Dict[str, Any] = {"id": None, "value": "", "type": "StrProperty"}

EMPTY_ENUM_ARRAY_PROPERTY: Dict[str, Any] = {
    "array_type": "EnumProperty",
    "id": None,
    "value": {"values": []},
    "type": "ArrayProperty",
}

EMPTY_BOOL_PROPERTY: Dict[str, Any] = {
    "value": False,
    "id": None,
    "type": "BoolProperty",
}

DYNAMIC_ITEM: Dict[str, Any] = {
    "ID": {
        "struct_type": "PalDynamicItemId",
        "struct_id": "00000000-0000-0000-0000-000000000000",
        "id": None,
        "value": {
            "CreatedWorldId": {
                "struct_type": "Guid",
                "struct_id": "00000000-0000-0000-0000-000000000000",
                "id": None,
                "value": "00000000-0000-0000-0000-000000000000",
                "type": "StructProperty",
            },
            "LocalIdInCreatedWorld": {
                "struct_type": "Guid",
                "struct_id": "00000000-0000-0000-0000-000000000000",
                "id": None,
                "value": None,
                "type": "StructProperty",
            },
        },
        "type": "StructProperty",
    },
    "StaticItemId": {
        "id": None,
        "value": None,
        "type": "NameProperty",
    },
    "RawData": {
        "array_type": "ByteProperty",
        "id": None,
        "value": {
            "id": {
                "created_world_id": "00000000-0000-0000-0000-000000000000",
                "local_id_in_created_world": None,
                "static_id": None,
            },
            "type": None,
            "durability": 0.0,
        },
        "type": "ArrayProperty",
        "custom_type": ".worldSaveData.DynamicItemSaveData.DynamicItemSaveData.RawData",
    },
    "CustomVersionData": {
        "array_type": "ByteProperty",
        "id": None,
        "value": {"values": [0, 0, 0, 0]},
        "type": "ArrayProperty",
    },
}

EMPTY_UUID = "00000000-0000-0000-0000-000000000000"


class PropertyType(Enum):
    STR = "StrProperty"
    ENUM_ARRAY = "EnumProperty"
    BOOL = "BoolProperty"
    DYNAMIC_ITEM = "DynamicItem"
    UUID = "UUID"


def get_empty_property(property_type: PropertyType):
    if property_type == PropertyType.STR:
        return copy.deepcopy(EMPTY_STR_PROPERTY)
    if property_type == PropertyType.ENUM_ARRAY:
        return copy.deepcopy(EMPTY_ENUM_ARRAY_PROPERTY)
    if property_type == PropertyType.BOOL:
        return copy.deepcopy(EMPTY_BOOL_PROPERTY)
    if property_type == PropertyType.DYNAMIC_ITEM:
        return copy.deepcopy(DYNAMIC_ITEM)
    if property_type == PropertyType.UUID:
        return copy.deepcopy(EMPTY_UUID)
