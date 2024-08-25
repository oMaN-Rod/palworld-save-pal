import traceback
from typing import Optional, Dict, Any, List, Tuple
from uuid import UUID
from pydantic import BaseModel, Field, field_validator, PrivateAttr

from palworld_save_tools.archive import UUID as ArchiveUUID

from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.save_file.utils import safe_get
from palworld_save_pal.save_file.enums import PalGender, Element
from palworld_save_pal.save_file.encoders import custom_uuid_encoder
from palworld_save_pal.save_file.pal_objects import *

from palworld_save_pal.save_file.empty_objects import get_empty_property, PropertyType

logger = create_logger(__name__)


def process_character_id(character_id: str, is_lucky: bool) -> Tuple[str, bool]:
    is_boss = False
    if not character_id:
        logger.error("Character ID is empty")
        return character_id, is_boss
    if character_id.lower().startswith("boss_"):
        character_id = character_id[5:]
        is_boss = not is_lucky
    if character_id.lower() == "lazycatfish":
        character_id = "LazyCatfish"
    if character_id.lower() == "sheepball":
        character_id = "Sheepball"
    return character_id, is_boss


class PalSummary(BaseModel):
    instance_id: UUID = Field(..., json_encoder=custom_uuid_encoder)
    character_id: str
    owner_uid: UUID
    nickname: Optional[str]
    level: int
    elements: List[str]


class Pal(BaseModel):
    instance_id: Optional[UUID] = None
    owner_uid: Optional[UUID] = None
    is_lucky: bool = False
    is_boss: bool = False
    character_id: Optional[str] = Field(None)
    gender: PalGender = PalGender.FEMALE
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

    _pal_obj: Dict[str, Any] = PrivateAttr(default_factory=dict)

    def __init__(self, **data):
        super().__init__(**data)
        data = data.get("data", {})
        if data:
            instance_id = (
                self.instance_id
                if self.instance_id
                else PalObjects.get_value(data["key"]["InstanceId"])
            )
            self.instance_id = instance_id.UUID() if isinstance(instance_id, ArchiveUUID) else instance_id
            self._pal_obj = PalObjects.get_nested(data, "value", "RawData", "value", "object", "SaveParameter", "value")
            if not self._pal_obj:
                logger.error("Failed to parse pal object: %s", data)
                return
            self._parse_pal_data()

    def _parse_pal_data(self):
        self.character_id = PalObjects.get_value(self._pal_obj["CharacterID"])
        if not self.character_id or self.character_id == "":
            logger.error("Failed to parse character ID: %s", self._pal_obj)
            return
        self.owner_uid = PalObjects.get_guid(self._pal_obj["OwnerPlayerUId"])
        self.is_lucky = PalObjects.get_value(self._pal_obj["IsRarePal"]) if "IsRarePal" in self._pal_obj else False
        self.work_speed = PalObjects.get_value(self._pal_obj["CraftSpeed"])
        self.nickname = PalObjects.get_value(self._pal_obj["NickName"]) if "NickName" in self._pal_obj else None

        self._process_character_id()
        self._get_gender()
        self._get_talents()
        self._get_ranks()
        self._get_level()
        self._get_storage_info()
        self._get_skills()
        self._get_work_suitabilities()
        self._get_hp()
        logger.info("Parsed PalEntity data: %s", self)

    def _process_character_id(self):
        self.character_id, self.is_boss = process_character_id(
            self.character_id, self.is_lucky
        )

    def _get_gender(self):
        gender = PalObjects.get_enum_property(self._pal_obj["Gender"])
        self.gender = PalGender.from_value(gender)

    def _get_talents(self):
        self.talent_hp = PalObjects.get_value(self._pal_obj["Talent_HP"], 0) if "Talent_HP" in self._pal_obj else 0
        self.talent_melee = PalObjects.get_value(self._pal_obj["Talent_Melee"], 0) if "Talent_Melee" in self._pal_obj else 0
        self.talent_shot = PalObjects.get_value(self._pal_obj["Talent_Shot"], 0) if "Talent_Shot" in self._pal_obj else 0
        self.talent_defense = PalObjects.get_value(self._pal_obj["Talent_Defense"], 0) if "Talent_Defense" in self._pal_obj else 0

    def _get_ranks(self):
        self.rank = PalObjects.get_value(self._pal_obj["Rank"], 1) if "Rank" in self._pal_obj else 1
        self.rank_hp = PalObjects.get_value(self._pal_obj["Rank_HP"], 0) if "Rank_HP" in self._pal_obj else 0
        self.rank_attack = PalObjects.get_value(self._pal_obj["Rank_Attack"], 0) if "Rank_Attack" in self._pal_obj else 0
        self.rank_defense = PalObjects.get_value(self._pal_obj["Rank_Defence"], 0) if "Rank_Defence" in self._pal_obj else 0
        self.rank_craftspeed = PalObjects.get_value(self._pal_obj["Rank_CraftSpeed"], 0) if "Rank_CraftSpeed" in self._pal_obj else 0
        if self.rank < 1 or self.rank > 5:
            self.rank = 1

    def _get_level(self):
        self.level = PalObjects.get_value(self._pal_obj["Level"], 1)

    def _get_storage_info(self):
        slot_id = PalObjects.get_value(self._pal_obj["SlotID"])
        if isinstance(slot_id, dict):
            self.storage_id = PalObjects.get_guid(PalObjects.get_nested(slot_id, "ContainerId", "value", "ID"))
            self.storage_slot = PalObjects.get_value(slot_id["SlotIndex"], 0)

    def _get_skills(self):
        self.learned_skills = PalObjects.get_array_property(self._pal_obj["MasteredWaza"]) if "MasteredWaza" in self._pal_obj else []
        self.passive_skills = PalObjects.get_array_property(self._pal_obj["PassiveSkillList"]) if "PassiveSkillList" in self._pal_obj else []
        equip_waza = PalObjects.get_array_property(self._pal_obj["EquipWaza"]) if "EquipWaza" in self._pal_obj else None
        self.active_skills = [skill.split("::")[-1] for skill in equip_waza] if equip_waza else []

    def _get_work_suitabilities(self):
        work_suitabilities = PalObjects.get_array_property(self._pal_obj["CraftSpeeds"])
        self.work_suitabilities = {}
        for ws in work_suitabilities:
            ws_type = WorkSuitability.from_value(PalObjects.get_enum_property(ws["WorkSuitability"]))
            ws_rank = PalObjects.get_value(ws["Rank"], 0)
            if ws_type:
                self.work_suitabilities[ws_type.value()] = ws_rank

    def _get_hp(self):
        self.hp = PalObjects.get_fixed_point64(self._pal_obj["Hp"])

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
            value = PalObjects.get_nested(data, "value", "RawData", "value", "object", "SaveParameter", "value", field)
            debug_info[field] = (
                str(value)[:100] if value is not None else "None"
            )  # Truncate long values
        logger.debug("Problematic pal data: %s", debug_info)

    @classmethod
    def create_summary(cls, data: Dict[str, Any]):
        instance_id = PalObjects.get_guid(PalObjects.get_nested(data, "key", "InstanceId"))
        if not instance_id:
            logger.error("Failed to parse instance ID: %s", data)
            return None
        
        pal_obj = PalObjects.get_nested(data, "value", "RawData", "value", "object", "SaveParameter", "value")
        if not pal_obj:
            logger.error("Failed to parse pal object: %s", data)
            return None
        
        pal_character_id = PalObjects.get_value(pal_obj["CharacterID"])
        is_lucky = PalObjects.get_value(pal_obj["IsRarePal"]) if "IsRarePal" in pal_obj else False
        pal_character_id, is_boss = process_character_id(pal_character_id, is_lucky)
        if not pal_character_id or pal_character_id == "":
            logger.error("Failed to parse character ID: %s", pal_obj)
            return None
        
        owner_uid = PalObjects.get_guid(pal_obj["OwnerPlayerUId"]) if "OwnerPlayerUId" in pal_obj else None
        nickname = PalObjects.get_value(pal_obj["NickName"]) if "NickName" in pal_obj else None
        level = PalObjects.get_value(pal_obj["Level"], 1) if "Level" in pal_obj else 1

        summary = cls(
            data={},
            instance_id=instance_id,
            is_lucky=is_lucky,
            is_boss=is_boss,
            character_id=pal_character_id,
            owner_uid=owner_uid or get_empty_property(PropertyType.UUID),
            nickname=nickname or "",
            level=level,
        )
        return summary
