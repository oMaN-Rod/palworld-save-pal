from enum import Enum
from typing import Any, Dict, List, Optional
from uuid import UUID

from pydantic import BaseModel, PrivateAttr, computed_field

from palworld_save_pal.game.pal_objects import ArrayType, PalGender, PalObjects
from palworld_save_pal.game.utils import clean_character_id
from palworld_save_pal.utils.dict import safe_remove, safe_remove_multiple
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class DynamicItemType(Enum):
    ARMOR = "armor"
    EGG = "egg"
    WEAPON = "weapon"


class DynamicItem(BaseModel):
    local_id: UUID

    _dynamic_item_save_data: Optional[Dict[str, Any]] = PrivateAttr(
        default_factory=dict
    )
    _raw_data: Optional[Dict[str, Any]] = PrivateAttr(default_factory=dict)

    def __init__(
        self, dynamic_item_save_data: Optional[Dict[str, Any]] = None, **kwargs: Any
    ):
        super().__init__(**kwargs)
        if dynamic_item_save_data is not None:
            self._dynamic_item_save_data = dynamic_item_save_data
            self._raw_data = dynamic_item_save_data["RawData"]["value"]

    @computed_field
    def character_id(self) -> Optional[str]:
        return PalObjects.get_nested(self._raw_data, "character_id")

    @character_id.setter
    def character_id(self, value: str) -> None:
        if self.type != DynamicItemType.EGG.value:
            safe_remove(self._raw_data, "character_id")
            return
        self._raw_data["character_id"] = value

    @computed_field
    def character_key(self) -> Optional[str]:
        if self.character_id is None:
            return None
        _, character_key = clean_character_id(self.character_id)
        return character_key

    @computed_field
    def durability(self) -> Optional[float]:
        return PalObjects.get_nested(self._raw_data, "durability")

    @durability.setter
    def durability(self, value: float) -> None:
        if self.type == DynamicItemType.EGG.value:
            safe_remove(self._raw_data, "durability")
            return
        self._raw_data["durability"] = value

    @computed_field
    def passive_skill_list(self) -> Optional[List[str]]:
        return PalObjects.get_nested(self._raw_data, "passive_skill_list")

    @passive_skill_list.setter
    def passive_skill_list(self, value: List[str]) -> None:
        if self.type != DynamicItemType.WEAPON.value:
            safe_remove(self._raw_data, "passive_skill_list")
            return
        self._raw_data["passive_skill_list"] = value if value else []

    @computed_field
    def remaining_bullets(self) -> Optional[int]:
        return PalObjects.get_nested(self._raw_data, "remaining_bullets")

    @remaining_bullets.setter
    def remaining_bullets(self, value: int) -> None:
        if self.type != DynamicItemType.WEAPON.value:
            safe_remove(self._raw_data, "remaining_bullets")
            return
        self._raw_data["remaining_bullets"] = value

    @computed_field
    def type(self) -> Optional[str]:
        return PalObjects.get_nested(self._raw_data, "type")

    @type.setter
    def type(self, value: str) -> None:
        self._raw_data["type"] = value

    @computed_field
    def static_id(self) -> Optional[str]:
        return PalObjects.get_nested(self._raw_data, "id", "static_id")

    @static_id.setter
    def static_id(self, value: str) -> None:
        self._raw_data["id"]["static_id"] = value

    @computed_field
    def gender(self) -> Optional[PalGender]:
        if not self._save_parameter:
            return None
        g = (
            PalObjects.get_enum_property(self._save_parameter["Gender"])
            if "Gender" in self._save_parameter
            else PalGender.FEMALE.prefixed()
        )
        return PalGender.from_value(g)

    @gender.setter
    def gender(self, value: PalGender):
        if not self._save_parameter:
            return
        self._save_parameter["Gender"] = PalObjects.EnumProperty(
            "EPalGenderType", value.prefixed()
        )

    @computed_field
    def active_skills(self) -> Optional[List[str]]:
        if not self._save_parameter:
            return None
        return (
            PalObjects.get_array_property(self._save_parameter["EquipWaza"])
            if "EquipWaza" in self._save_parameter
            else []
        )

    @active_skills.setter
    def active_skills(self, value: List[str] = []):
        if not self._save_parameter:
            return
        self._save_parameter["EquipWaza"] = PalObjects.ArrayPropertyValues(
            ArrayType.ENUM_PROPERTY, value
        )

    @computed_field
    def learned_skills(self) -> Optional[List[str]]:
        if not self._save_parameter:
            return None
        return (
            PalObjects.get_array_property(self._save_parameter["MasteredWaza"])
            if "MasteredWaza" in self._save_parameter
            else []
        )

    @learned_skills.setter
    def learned_skills(self, value: List[str] = []):
        if not self._save_parameter:
            return
        self._save_parameter["MasteredWaza"] = PalObjects.ArrayPropertyValues(
            ArrayType.ENUM_PROPERTY, value
        )

    @computed_field
    def passive_skills(self) -> Optional[List[str]]:
        if not self._save_parameter:
            return None
        return (
            PalObjects.get_array_property(self._save_parameter["PassiveSkillList"])
            if "PassiveSkillList" in self._save_parameter
            else []
        )

    @passive_skills.setter
    def passive_skills(self, value: List[str] = []):
        if not self._save_parameter:
            return
        self._save_parameter["PassiveSkillList"] = PalObjects.ArrayPropertyValues(
            ArrayType.NAME_PROPERTY, value
        )

    @computed_field
    def talent_hp(self) -> Optional[int]:
        if not self._save_parameter:
            return None
        return (
            PalObjects.get_byte_property(self._save_parameter["Talent_HP"])
            if "Talent_HP" in self._save_parameter
            else 0
        )

    @talent_hp.setter
    def talent_hp(self, value: int):
        if not self._save_parameter:
            return
        self._save_parameter["Talent_HP"] = PalObjects.ByteProperty(value)

    @computed_field
    def talent_shot(self) -> Optional[int]:
        if not self._save_parameter:
            return None
        return (
            PalObjects.get_byte_property(self._save_parameter["Talent_Shot"])
            if "Talent_Shot" in self._save_parameter
            else 0
        )

    @talent_shot.setter
    def talent_shot(self, value: int):
        if not self._save_parameter:
            return
        self._save_parameter["Talent_Shot"] = PalObjects.ByteProperty(value)

    @computed_field
    def talent_defense(self) -> Optional[int]:
        if not self._save_parameter:
            return None
        return (
            PalObjects.get_byte_property(self._save_parameter["Talent_Defense"])
            if "Talent_Defense" in self._save_parameter
            else 0
        )

    @talent_defense.setter
    def talent_defense(self, value: int):
        if not self._save_parameter:
            return
        self._save_parameter["Talent_Defense"] = PalObjects.ByteProperty(value)

    @property
    def _save_parameter(self) -> Dict[str, Any]:
        return PalObjects.get_nested(self._raw_data, "object", "SaveParameter", "value")

    @property
    def save_data(self) -> Dict[str, Any]:
        return self._dynamic_item_save_data

    def update_from(self, other: Dict[str, Any]) -> None:
        self.type = other["type"]
        match self.type:
            case DynamicItemType.ARMOR.value:
                safe_remove_multiple(
                    self._raw_data,
                    "object",
                    "unknown_bytes",
                    "unknown_id",
                    "remaining_bullets",
                    "passive_skill_list",
                )
            case DynamicItemType.EGG.value:
                if "object" not in self._raw_data:
                    self._raw_data["object"] = {}
                self.character_id = other["character_id"]
                if "modified" in other and other["modified"]:
                    self._raw_data["object"] = PalObjects.SaveParameter(
                        character_id=other["character_id"],
                        gender=PalGender.from_value(other["gender"]),
                        active_skills=other["active_skills"],
                        learned_skills=other["learned_skills"],
                        passive_skills=other["passive_skills"],
                        talent_hp=other["talent_hp"],
                        talent_shot=other["talent_shot"],
                        talent_defense=other["talent_defense"],
                    )
                self._raw_data["unknown_bytes"] = [0, 0, 0, 0]
                self._raw_data["unknown_id"] = PalObjects.EMPTY_UUID
                safe_remove_multiple(
                    self._raw_data,
                    "durability",
                    "remaining_bullets",
                    "passive_skill_list",
                )
                return
            case DynamicItemType.WEAPON.value:
                safe_remove_multiple(
                    self._raw_data, "object", "unknown_bytes", "unknown_id"
                )
                if "passive_skill_list" not in self._raw_data:
                    self.passive_skill_list = []

        type_converters = {
            "gender": lambda x: PalGender.from_value(x) if x else None,
            "hp": int,
            "talent_hp": int,
            "talent_shot": int,
            "talent_defense": int,
            "learned_skills": list,
            "active_skills": list,
            "passive_skills": list,
        }

        for key, value in other.items():
            if key == "type":
                continue
            if hasattr(self, key):
                try:
                    if key in type_converters:
                        converted_value = type_converters[key](value)
                        setattr(self, key, converted_value)
                    else:
                        setattr(self, key, value)
                except Exception as e:
                    logger.warning(f"Failed to update {key}: {str(e)}")
                    continue
