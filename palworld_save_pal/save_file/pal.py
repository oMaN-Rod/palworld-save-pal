import copy
from typing import Optional, Dict, Any, List
from uuid import UUID
from pydantic import BaseModel, Field, PrivateAttr


from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.save_file.pal_objects import *


logger = create_logger(__name__)


class Pal(BaseModel):
    instance_id: Optional[UUID] = None
    owner_uid: Optional[UUID] = None
    is_lucky: bool = False
    is_boss: bool = False
    character_id: Optional[str] = Field(None)
    gender: Optional[PalGender] = None
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
    state: EntryState = EntryState.NONE

    _character_save: Dict[str, Any] = PrivateAttr(default_factory=dict)
    _save_parameter: Dict[str, Any] = PrivateAttr(default_factory=dict)

    def __init__(self, data=None, **kwargs):
        if data is not None:
            super().__init__()
            self.instance_id = PalObjects.get_guid(data["key"]["InstanceId"])
            if not self.instance_id:
                logger.error("Failed to parse instance ID: %s", data)

            self._character_save = data
            self._save_parameter = PalObjects.get_nested(
                self._character_save,
                "value",
                "RawData",
                "value",
                "object",
                "SaveParameter",
                "value",
            )
            if not self._save_parameter:
                logger.error("Failed to parse pal object: %s", data)
                return
            self._parse_pal_data()
        else:
            super().__init__(**kwargs)

    def clone(self, instance_id: UUID, slot_idx: int, nickname: str) -> "Pal":
        new_pal = copy.deepcopy(self)
        new_pal.instance_id = instance_id
        new_pal._update_instance_id()
        new_pal.nickname = nickname
        new_pal._update_nickname()
        new_pal._update_slot_idx(slot_idx)
        return new_pal

    def character_save(self) -> Dict[str, Any]:
        return self._character_save

    def update(self):
        self._update_nickname()
        self._update_gender()
        self._update_equip_waza()
        self._update_mastered_waza()
        self._update_passive_skills()

    def update_from(self, other_pal: "Pal"):
        data = other_pal.model_dump()
        for key, value in data.items():
            setattr(self, key, value)
        self.update()

    def _parse_pal_data(self):
        self.character_id = PalObjects.get_value(self._save_parameter["CharacterID"])
        if not self.character_id or self.character_id == "":
            logger.error("Failed to parse character ID: %s", self._save_parameter)
            return
        self.owner_uid = (
            PalObjects.get_guid(self._save_parameter["OwnerPlayerUId"])
            if "OwnerPlayerUId" in self._save_parameter
            else None
        )
        self.is_lucky = (
            PalObjects.get_value(self._save_parameter["IsRarePal"])
            if "IsRarePal" in self._save_parameter
            else False
        )
        self.work_speed = PalObjects.get_value(self._save_parameter["CraftSpeed"])
        self.nickname = (
            PalObjects.get_value(self._save_parameter["NickName"])
            if "NickName" in self._save_parameter
            else None
        )

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
        self.is_boss = False
        if self.character_id.lower().startswith("boss_"):
            self.character_id = self.character_id[5:]
            self.is_boss = not self.is_lucky
        if self.character_id.lower() == "lazycatfish":
            self.character_id = "LazyCatfish"
        if self.character_id.lower() == "sheepball":
            self.character_id = "Sheepball"

    def _get_gender(self):
        gender = (
            PalObjects.get_enum_property(self._save_parameter["Gender"])
            if "Gender" in self._save_parameter
            else None
        )
        self.gender = PalGender.from_value(gender) if gender else None

    def _get_talents(self):
        self.talent_hp = (
            PalObjects.get_value(self._save_parameter["Talent_HP"], 0)
            if "Talent_HP" in self._save_parameter
            else 0
        )
        self.talent_melee = (
            PalObjects.get_value(self._save_parameter["Talent_Melee"], 0)
            if "Talent_Melee" in self._save_parameter
            else 0
        )
        self.talent_shot = (
            PalObjects.get_value(self._save_parameter["Talent_Shot"], 0)
            if "Talent_Shot" in self._save_parameter
            else 0
        )
        self.talent_defense = (
            PalObjects.get_value(self._save_parameter["Talent_Defense"], 0)
            if "Talent_Defense" in self._save_parameter
            else 0
        )

    def _get_ranks(self):
        self.rank = (
            PalObjects.get_value(self._save_parameter["Rank"], 1)
            if "Rank" in self._save_parameter
            else 1
        )
        self.rank_hp = (
            PalObjects.get_value(self._save_parameter["Rank_HP"], 0)
            if "Rank_HP" in self._save_parameter
            else 0
        )
        self.rank_attack = (
            PalObjects.get_value(self._save_parameter["Rank_Attack"], 0)
            if "Rank_Attack" in self._save_parameter
            else 0
        )
        self.rank_defense = (
            PalObjects.get_value(self._save_parameter["Rank_Defence"], 0)
            if "Rank_Defence" in self._save_parameter
            else 0
        )
        self.rank_craftspeed = (
            PalObjects.get_value(self._save_parameter["Rank_CraftSpeed"], 0)
            if "Rank_CraftSpeed" in self._save_parameter
            else 0
        )
        if self.rank < 1 or self.rank > 5:
            self.rank = 1

    def _get_level(self):
        self.level = (
            PalObjects.get_value(self._save_parameter["Level"], 1)
            if "Level" in self._save_parameter
            else 1
        )

    def _get_storage_info(self):
        slot_id = PalObjects.get_value(self._save_parameter["SlotID"])
        if isinstance(slot_id, dict):
            self.storage_id = PalObjects.get_guid(
                PalObjects.get_nested(slot_id, "ContainerId", "value", "ID")
            )
            self.storage_slot = PalObjects.get_value(slot_id["SlotIndex"], 0)

    def _get_skills(self):
        self.learned_skills = (
            PalObjects.get_array_property(self._save_parameter["MasteredWaza"])
            if "MasteredWaza" in self._save_parameter
            else []
        )
        self.passive_skills = (
            PalObjects.get_array_property(self._save_parameter["PassiveSkillList"])
            if "PassiveSkillList" in self._save_parameter
            else []
        )
        equip_waza = (
            PalObjects.get_array_property(self._save_parameter["EquipWaza"])
            if "EquipWaza" in self._save_parameter
            else None
        )
        self.active_skills = (
            [skill.split("::")[-1] for skill in equip_waza] if equip_waza else []
        )

    def _get_work_suitabilities(self):
        work_suitabilities = PalObjects.get_array_property(
            self._save_parameter["CraftSpeeds"]
        )
        self.work_suitabilities = {}
        for ws in work_suitabilities:
            ws_type = WorkSuitability.from_value(
                PalObjects.get_enum_property(ws["WorkSuitability"])
            )
            ws_rank = PalObjects.get_value(ws["Rank"], 0)
            if ws_type:
                self.work_suitabilities[ws_type.value] = ws_rank

    def _get_hp(self):
        self.hp = (
            PalObjects.get_fixed_point64(self._save_parameter["Hp"])
            if "Hp" in self._save_parameter
            else 0
        )

    def _update_instance_id(self):
        PalObjects.set_value(
            self._character_save["key"]["InstanceId"], value=self.instance_id
        )

    def _update_nickname(self):
        if not self.nickname or len(self.nickname) == 0:
            return
        if "NickName" in self._save_parameter:
            PalObjects.set_value(self._save_parameter["NickName"], value=self.nickname)
        else:
            self._save_parameter["NickName"] = PalObjects.StrProperty(self.nickname)

    def _update_gender(self):
        self.gender = self.gender if self.gender else PalGender.FEMALE
        if "Gender" in self._save_parameter:
            PalObjects.set_enum_property(
                self._save_parameter["Gender"], value=self.gender.prefixed()
            )
        else:
            self._save_parameter["Gender"] = PalObjects.EnumProperty(
                "EPalGenderType", self.gender.prefixed()
            )

    def _update_equip_waza(self):
        if not self.active_skills or len(self.active_skills) == 0:
            active_skills = []
        else:
            active_skills = [f"EPalWazaID::{skill}" for skill in self.active_skills]

        if "EquipWaza" in self._save_parameter:
            logger.debug(
                "Updating active skills: %s, %s", active_skills, self._save_parameter
            )
            PalObjects.set_array_property(
                self._save_parameter["EquipWaza"], values=active_skills
            )
        else:
            self._save_parameter["EquipWaza"] = PalObjects.ArrayPropertyValues(
                ArrayType.ENUM_PROPERTY, active_skills
            )

    def _update_mastered_waza(self):
        if not self.learned_skills or len(self.learned_skills) == 0:
            learned_skills = []
        else:
            learned_skills = self.learned_skills

        if "MasteredWaza" in self._save_parameter:
            PalObjects.set_array_property(
                self._save_parameter["MasteredWaza"], values=learned_skills
            )
        else:
            self._save_parameter["MasteredWaza"] = PalObjects.ArrayPropertyValues(
                ArrayType.ENUM_PROPERTY, learned_skills
            )

    def _update_passive_skills(self) -> None:
        if "PassiveSkillList" in self._save_parameter:
            PalObjects.set_array_property(
                self._save_parameter["PassiveSkillList"], values=self.passive_skills
            )
        else:
            self._save_parameter["PassiveSkillList"] = PalObjects.ArrayPropertyValues(
                ArrayType.NAME_PROPERTY, self.passive_skills
            )

    def _update_slot_idx(self, slot_idx: int) -> None:
        PalObjects.set_value(
            PalObjects.get_nested(self._save_parameter, "SlotID", "value", "SlotIndex"),
            value=slot_idx,
        )
