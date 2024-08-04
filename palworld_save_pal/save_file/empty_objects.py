from typing import Dict, Any

# Empty skill object template
EMPTY_SKILL_OBJECT: Dict[str, Any] = {
    "array_type": "NameProperty",
    "id": None,
    "value": {"values": []},
    "type": "ArrayProperty",
}

EMPTY_STR_PROPERTY: Dict[str, Any] = {"id": None, "value": "", "type": "StrProperty"}

EMPTY_INT_PROPERTY: Dict[str, Any] = {"id": None, "value": 0, "type": "IntProperty"}

EMPTY_ENUM_ARRAY_PROPERTY: Dict[str, Any] = {
    "array_type": "EnumProperty",
    "id": None,
    "value": {"values": []},
    "type": "ArrayProperty",
}

# Empty level object template
EMPTY_LEVEL_OBJECT: Dict[str, Any] = {"id": None, "value": 1, "type": "IntProperty"}

# Empty rank object template
EMPTY_RANK_OBJECT: Dict[str, Any] = {"id": None, "value": 1, "type": "IntProperty"}

# Empty melee object template
EMPTY_MELEE_OBJECT: Dict[str, Any] = {"id": None, "value": 0, "type": "IntProperty"}

# Empty shot object template
EMPTY_SHOT_OBJECT: Dict[str, Any] = {"id": None, "value": 0, "type": "IntProperty"}

# Empty defense object template
EMPTY_DEFENSE_OBJECT: Dict[str, Any] = {"id": None, "value": 0, "type": "IntProperty"}

# Empty experience object template
EMPTY_EXP_OBJECT: Dict[str, Any] = {"id": None, "value": 0, "type": "IntProperty"}

# Empty HP object template
EMPTY_HP_OBJECT: Dict[str, Any] = {
    "struct_type": "FixedPoint64",
    "struct_id": "00000000-0000-0000-0000-000000000000",
    "id": None,
    "value": {"Value": {"id": None, "value": 0, "type": "Int64Property"}},
    "type": "StructProperty",
}

# Empty rare Pal object template
EMPTY_RARE_PAL_OBJECT: Dict[str, Any] = {
    "value": False,
    "id": None,
    "type": "BoolProperty",
}

# Empty moves object template
EMPTY_MOVES_OBJECT: Dict[str, Any] = {
    "array_type": "EnumProperty",
    "id": None,
    "value": {"values": []},
    "type": "ArrayProperty",
}

# Empty suit object template
EMPTY_SUIT_OBJECT: Dict[str, Any] = {
    "array_type": "StructProperty",
    "id": None,
    "value": {
        "prop_name": "CraftSpeeds",
        "prop_type": "StructProperty",
        "values": [
            {
                "WorkSuitability": {
                    "id": None,
                    "value": {
                        "type": "EPalWorkSuitability",
                        "value": "EPalWorkSuitability::EmitFlame",
                    },
                    "type": "EnumProperty",
                },
                "Rank": {"id": None, "value": 0, "type": "IntProperty"},
            },
            # ... (other work suitability entries)
        ],
        "type_name": "PalWorkSuitabilityInfo",
        "id": "00000000-0000-0000-0000-000000000000",
    },
    "type": "ArrayProperty",
}

EMPTY_PALWORLD_UUID = "00000000-0000-0000-0000-000000000000"
