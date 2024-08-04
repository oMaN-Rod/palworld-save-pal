import traceback
from typing import Optional, Dict, Any, List, Tuple
from uuid import UUID
from pydantic import BaseModel, Field, field_validator, PrivateAttr

from palworld_save_tools.archive import UUID as ArchiveUUID

from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.save_file.utils import safe_get
from palworld_save_pal.pals.models import PalGender, Element

from palworld_save_pal.save_file.empty_objects import EMPTY_PALWORLD_UUID

logger = create_logger(__name__)


def process_character_id(character_id: str, is_lucky: bool) -> Tuple[str, bool]:
    is_boss = False
    if character_id.lower().startswith("boss_"):
        character_id = character_id[5:]
        is_boss = not is_lucky
    if character_id.lower() == "lazycatfish":
        character_id = "LazyCatfish"
    if character_id.lower() == "sheepball":
        character_id = "Sheepball"
    return character_id, is_boss


class Pal(BaseModel):
    instance_id: Optional[UUID] = None
    owner_uid: Optional[UUID] = None
    is_lucky: bool = False
    is_boss: bool = False
    character_id: Optional[str] = Field(None)
    gender: PalGender = PalGender.UNKNOWN
    work_speed: float = Field(0.0)
    rank_hp: int = 0
    rank_attack: int = 0
    rank_defense: int = 0
    rank_craftspeed: int = 0
    talent_hp: int = 0
    talent_melee: int = 0
    talent_shot: int = 0
    talent_defense: int = 0
    rank: int = 1
    level: int = 1
    nickname: Optional[str] = ""
    is_tower: bool = False
    storage_id: Optional[UUID] = None
    storage_slot: int = 0
    learned_skills: List[str] = Field(default_factory=list)
    active_skills: List[str] = Field(default_factory=list)
    passive_skills: List[str] = Field(default_factory=list)
    work_suitabilities: Optional[Dict[str, int]] = Field(default_factory=dict)
    hp: int = 0
    max_hp: int = 0
    elements: List[Element] = Field(default_factory=list)

    _data: Dict[str, Any] = PrivateAttr(default_factory=dict)
    _pal_obj: Dict[str, Any] = PrivateAttr(default_factory=dict)

    def __init__(self, **data):
        super().__init__(**data)
        self._data = data.get("data", {})
        if self._data:
            instance_id = (
                self.instance_id
                if self.instance_id
                else safe_get(self._data, "key", "InstanceId", "value", default=None)
            )
            self.instance_id = instance_id.UUID()
            self._pal_obj = safe_get(
                self._data,
                "value",
                "RawData",
                "value",
                "object",
                "SaveParameter",
                "value",
                default={},
            )
            self._parse_pal_data()

    def _parse_pal_data(self):
        self.character_id = self._parse_str(self._extract_value("CharacterID"))
        if not self.character_id or self.character_id == "":
            logger.error("Failed to parse character ID: %s", self._pal_obj)
            return

        self.owner_uid = self._parse_uuid(self._extract_value("OwnerPlayerUId"))
        self.is_lucky = self._parse_bool(self._extract_value("IsRarePal"))
        self.work_speed = self._parse_float(self._extract_value("CraftSpeed"), 0.0)
        self.nickname = self._parse_str(self._extract_value("NickName"))

        self._process_character_id()
        self._set_gender()
        self._set_talents()
        self._set_rank()
        self._set_level()
        self._set_storage_info()
        self._set_skills()
        self._set_work_suitabilities()
        self._set_hp()
        self._pal_obj = {}
        self._data = {}
        logger.info("Parsed PalEntity data: %s", self)

    def _extract_value(self, key: str, d: Dict[str, Any] | None = None) -> Any:
        target = d or self._pal_obj
        value = safe_get(target, key, "value", default=None)
        if isinstance(value, dict):
            if "value" in value:
                return value["value"]
            elif "values" in value:
                return value["values"]
        return value

    def _process_character_id(self):
        self.character_id, self.is_boss = process_character_id(
            self.character_id, self.is_lucky
        )

    def _set_gender(self):
        gender_value = self._extract_value("Gender")
        if isinstance(gender_value, dict) and "value" in gender_value:
            gender_value = gender_value["value"]
        if gender_value == "EPalGenderType::Male":
            self.gender = PalGender.MALE
        elif gender_value == "EPalGenderType::Female":
            self.gender = PalGender.FEMALE

    def _set_talents(self):
        self.rank_hp = self._parse_int(self._extract_value("Rank_HP"), 0)
        self.rank_attack = self._parse_int(self._extract_value("Rank_Attack"), 0)
        self.rank_defense = self._parse_int(self._extract_value("Rank_Defence"), 0)
        self.rank_craftspeed = self._parse_int(
            self._extract_value("Rank_CraftSpeed"), 0
        )
        self.talent_hp = self._parse_int(self._extract_value("Talent_HP"), 0)
        self.talent_melee = self._parse_int(self._extract_value("Talent_Melee"), 0)
        self.talent_shot = self._parse_int(self._extract_value("Talent_Shot"), 0)
        self.talent_defense = self._parse_int(self._extract_value("Talent_Defense"), 0)

    def _set_rank(self):
        self.rank = self._parse_int(self._extract_value("Rank"), 1)
        if self.rank < 1 or self.rank > 5:
            self.rank = 1

    def _set_level(self):
        self.level = self._parse_int(self._extract_value("Level"), 1)

    def _set_storage_info(self):
        slot_id = self._extract_value("SlotID")
        if isinstance(slot_id, dict):
            self.storage_id = self._parse_uuid(
                safe_get(slot_id, "ContainerId", "value", "ID", "value")
            )
            self.storage_slot = self._parse_int(
                safe_get(slot_id, "SlotIndex", "value"), 0
            )

    def _set_skills(self):
        self.learned_skills = self._parse_list(self._extract_value("MasteredWaza"))
        self.passive_skills = self._parse_list(self._extract_value("PassiveSkillList"))
        equip_waza = self._parse_list(self._extract_value("EquipWaza"))
        self.active_skills = [skill.split("::")[-1] for skill in equip_waza]

    def _set_work_suitabilities(self):
        work_suitabilities = self._parse_list(self._extract_value("CraftSpeeds"))
        self.work_suitabilities = {}
        for ws in work_suitabilities:
            ws_type = safe_get(ws, "WorkSuitability", "value", "value", default="")
            if "::" in ws_type:
                ws_type = ws_type.split("::")[-1]
            value = safe_get(ws, "Rank", "value", default=0)
            if ws_type:
                self.work_suitabilities[ws_type] = value

    def _set_hp(self):
        hp_value = self._extract_value("HP")
        if isinstance(hp_value, dict) and "Value" in hp_value:
            self.hp = self._parse_int(hp_value["Value"].get("value"), 0)
        else:
            self.hp = self._parse_int(hp_value, 0)

    @staticmethod
    def _parse_int(value: Any, default: int = 0) -> int:
        try:
            return int(value)
        except (TypeError, ValueError):
            return default

    @staticmethod
    def _parse_float(value: Any, default: float = 0.0) -> float:
        try:
            return float(value)
        except (TypeError, ValueError):
            return default

    @staticmethod
    def _parse_str(value: Any, default: str = "") -> str:
        return str(value) if value is not None else default

    @staticmethod
    def _parse_bool(value: Any, default: bool = False) -> bool:
        return bool(value) if value is not None else default

    @staticmethod
    def _parse_list(value: Any, default: List = None) -> List:
        if isinstance(value, list):
            return value
        return [] if default is None else default

    @staticmethod
    def _parse_uuid(value: Any) -> Optional[UUID]:
        if isinstance(value, UUID):
            return value
        if isinstance(value, ArchiveUUID):
            return value.UUID()
        if isinstance(value, str):
            return UUID(value)
        if isinstance(value, dict):
            return Pal._parse_uuid(
                value.get("value", "00000000-0000-0000-0000-000000000000")
            )
        return None

    @field_validator("rank")
    @classmethod
    def validate_rank(cls, v):
        return max(1, min(v, 5))

    @classmethod
    def create_safe(cls, data: Dict[str, Any]):
        try:
            pal = cls(data=data)
            return pal
        except Exception as e:
            logger.error("Error creating PalEntity: %s", str(e))
            cls._log_problematic_data(data)
            traceback.print_exc()
            return None

    @staticmethod
    def _log_problematic_data(data: Dict[str, Any]):
        problematic_fields = [
            "CharacterID",
            "CraftSpeed",
            "SlotID",
            "OwnerPlayerUId",
            "IsRarePal",
            "Gender",
            "Talent_HP",
            "Talent_Melee",
            "Talent_Shot",
            "Talent_Defense",
            "Rank",
            "Level",
            "NickName",
        ]
        debug_info = {}
        for field in problematic_fields:
            value = safe_get(
                data,
                "value",
                "RawData",
                "value",
                "object",
                "SaveParameter",
                "value",
                field,
            )
            debug_info[field] = (
                str(value)[:100] if value is not None else "None"
            )  # Truncate long values
        logger.debug("Problematic pal data: %s", debug_info)

    @classmethod
    def create_summary(cls, data: Dict[str, Any]):
        def extract_value(key: str, d: Dict[str, Any] | None = None) -> Any:
            value = safe_get(d, key, "value", default=None)
            if isinstance(value, dict):
                if "value" in value:
                    return value["value"]
                elif "values" in value:
                    return value["values"]
            return value

        instance_id = cls._parse_uuid(safe_get(data, "key", "InstanceId", "value"))
        pal_obj = safe_get(
            data,
            "value",
            "RawData",
            "value",
            "object",
            "SaveParameter",
            "value",
            default={},
        )
        is_lucky = cls._parse_bool(extract_value("IsRarePal"))
        pal_character_id = cls._parse_str(extract_value("CharacterID", pal_obj))
        pal_character_id, is_boss = process_character_id(pal_character_id, is_lucky)
        owner_uid = cls._parse_uuid(extract_value("OwnerPlayerUId", pal_obj))
        nickname = cls._parse_str(extract_value("NickName", pal_obj))
        level = cls._parse_int(extract_value("Level", pal_obj), 1)

        if not instance_id:
            logger.error("Failed to parse instance ID: %s", data)
            return None
        if not pal_character_id or pal_character_id == "":
            logger.error("Failed to parse character ID: %s", pal_obj)
            return None

        summary = cls(
            data={},
            instance_id=instance_id,
            is_lucky=is_lucky,
            is_boss=is_boss,
            character_id=pal_character_id,
            owner_uid=owner_uid or EMPTY_PALWORLD_UUID,
            nickname=nickname or "",
            level=level,
        )
        return summary
