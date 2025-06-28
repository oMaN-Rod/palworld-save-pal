import copy
import math
from typing import Optional, Dict, Any, List
from uuid import UUID
from pydantic import BaseModel, PrivateAttr, computed_field

from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.game.utils import clean_character_id
from palworld_save_pal.utils.dict import safe_remove
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.game.pal_objects import (
    PalObjects,
    ArrayType,
    WorkSuitability,
    PalGender,
)


logger = create_logger(__name__)
PAL_DATA = JsonManager("data/json/pals.json").read()

PAL_SICK_TYPES = [
    "PalReviveTimer",
    "PhysicalHealth",
    "WorkerSick",
    "HungerType",
    "SanityValue",
]


class Pal(BaseModel):
    _is_dps: bool = PrivateAttr(default=False)
    _character_save: Dict[str, Any] = PrivateAttr(default_factory=dict)
    _save_parameter: Dict[str, Any] = PrivateAttr(default_factory=dict)

    def __init__(self, data=None, dps=False, **kwargs):
        if data is not None and not dps:
            super().__init__()
            self._is_dps = dps
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
        elif data is not None and dps:
            super().__init__()
            self._is_dps = dps
            self._character_save = data
            self._save_parameter = PalObjects.get_value(
                self._character_save["SaveParameter"]
            )
        else:
            super().__init__(**kwargs)

    @computed_field
    def instance_id(self) -> UUID:
        return PalObjects.get_guid(
            self._character_save["InstanceId"]["value"]["InstanceId"]
            if self._is_dps
            else self._character_save["key"]["InstanceId"]
        )

    @instance_id.setter
    def instance_id(self, value: UUID):
        PalObjects.set_value(
            self._character_save["InstanceId"]["value"]["InstanceId"]
            if self._is_dps
            else self._character_save["key"]["InstanceId"],
            value=value,
        )

    @computed_field
    def character_id(self) -> Optional[str]:
        return (
            PalObjects.get_value(self._save_parameter["CharacterID"])
            if "CharacterID" in self._save_parameter
            else None
        )

    @character_id.setter
    def character_id(self, value: str):
        PalObjects.set_value(self._save_parameter["CharacterID"], value=value)

    @computed_field
    def character_key(self) -> Optional[str]:
        self.character_id, character_key = clean_character_id(self.character_id)
        return character_key

    @computed_field
    def owner_uid(self) -> Optional[UUID]:
        return (
            PalObjects.get_guid(self._save_parameter["OwnerPlayerUId"])
            if "OwnerPlayerUId" in self._save_parameter
            else None
        )

    @owner_uid.setter
    def owner_uid(self, value: UUID):
        self._save_parameter["OwnerPlayerUId"] = PalObjects.Guid(value)

    @computed_field
    def is_lucky(self) -> bool:
        return (
            PalObjects.get_value(self._save_parameter["IsRarePal"])
            if "IsRarePal" in self._save_parameter
            else False
        )

    @is_lucky.setter
    def is_lucky(self, value: bool):
        if value:
            self._save_parameter["IsRarePal"] = PalObjects.BoolProperty(value)
        else:
            safe_remove(self._save_parameter, "IsRarePal")
        self._format_boss_character_id(value)

    @computed_field
    def is_boss(self) -> bool:
        return self.character_id.upper().startswith("BOSS_") and not self.is_lucky

    @is_boss.setter
    def is_boss(self, value: bool):
        self._format_boss_character_id(value)

    @computed_field
    def is_predator(self) -> bool:
        return self.character_id.startswith("PREDATOR_") if self.character_id else False

    @computed_field
    def is_tower(self) -> bool:
        return self.character_id.startswith("GYM_") if self.character_id else False

    @computed_field
    def gender(self) -> Optional[PalGender]:
        g = (
            PalObjects.get_enum_property(self._save_parameter["Gender"])
            if "Gender" in self._save_parameter
            else PalGender.FEMALE.prefixed()
        )
        return PalGender.from_value(g)

    @gender.setter
    def gender(self, value: PalGender):
        self._save_parameter["Gender"] = PalObjects.EnumProperty(
            "EPalGenderType", value.prefixed()
        )

    @computed_field
    def nickname(self) -> Optional[str]:
        return (
            PalObjects.get_value(self._save_parameter["NickName"])
            if "NickName" in self._save_parameter
            else None
        )

    @nickname.setter
    def nickname(self, value: str):
        self._save_parameter["NickName"] = PalObjects.StrProperty(value)

    @computed_field
    def filtered_nickname(self) -> Optional[str]:
        return (
            PalObjects.get_value(self._save_parameter["FilteredNickName"])
            if self._is_dps and "FilteredNickName" in self._save_parameter
            else None
        )

    @filtered_nickname.setter
    def filtered_nickname(self, value: str):
        if self._is_dps:
            self._save_parameter["FilteredNickName"] = PalObjects.StrProperty(value)

    @computed_field
    def group_id(self) -> Optional[UUID]:
        return PalObjects.as_uuid(
            PalObjects.get_nested(
                self._character_save, "value", "RawData", "value", "group_id"
            )
        )

    @group_id.setter
    def group_id(self, value: UUID):
        if "group_id" in self._character_save["value"]["RawData"]["value"]:
            PalObjects.set_nested(
                self._character_save,
                "value",
                "RawData",
                "value",
                "group_id",
                value=value,
            )

    @computed_field
    def stomach(self) -> float:
        return (
            PalObjects.get_value(self._save_parameter["FullStomach"], 150)
            if "FullStomach" in self._save_parameter
            else 150
        )

    @stomach.setter
    def stomach(self, value: float):
        self._save_parameter["FullStomach"] = PalObjects.FloatProperty(value)

    @computed_field
    def sanity(self) -> float:
        return (
            PalObjects.get_value(self._save_parameter["SanityValue"], 100.0)
            if "SanityValue" in self._save_parameter
            else 100.0
        )

    @sanity.setter
    def sanity(self, value: float):
        self._save_parameter["SanityValue"] = PalObjects.FloatProperty(value)

    @computed_field
    def hp(self) -> int:
        if "HP" in self._save_parameter:
            self._save_parameter["Hp"] = self._save_parameter.pop("HP")
        return (
            PalObjects.get_fixed_point64(self._save_parameter["Hp"])
            if "Hp" in self._save_parameter
            else 0
        )

    @hp.setter
    def hp(self, value: int):
        self._save_parameter["Hp"] = PalObjects.FixedPoint64(value)

    @computed_field
    def level(self) -> int:
        return (
            PalObjects.get_byte_property(self._save_parameter["Level"])
            if "Level" in self._save_parameter
            else 1
        )

    @level.setter
    def level(self, value: int):
        if value <= 1:
            safe_remove(self._save_parameter, "Level")
            return
        self._save_parameter["Level"] = PalObjects.ByteProperty(value)

    @computed_field
    def exp(self) -> int:
        return (
            PalObjects.get_value(self._save_parameter["Exp"])
            if "Exp" in self._save_parameter
            else 0
        )

    @exp.setter
    def exp(self, value: int):
        if value == 0:
            safe_remove(self._save_parameter, "Exp")
            return
        self._save_parameter["Exp"] = PalObjects.Int64Property(value)

    @computed_field
    def rank(self) -> int:
        return (
            int(PalObjects.get_byte_property(self._save_parameter["Rank"]))
            if "Rank" in self._save_parameter
            else 0
        )

    @rank.setter
    def rank(self, value: int):
        value = min(value, 255)
        if value == 0:
            safe_remove(self._save_parameter, "Rank")
            return
        self._save_parameter["Rank"] = PalObjects.ByteProperty(value)

    @computed_field
    def rank_hp(self) -> int:
        return (
            PalObjects.get_byte_property(self._save_parameter["Rank_HP"])
            if "Rank_HP" in self._save_parameter
            else 0
        )

    @rank_hp.setter
    def rank_hp(self, value: int):
        if value == 0:
            safe_remove(self._save_parameter, "Rank_HP")
            return
        self._save_parameter["Rank_HP"] = PalObjects.ByteProperty(value)

    @computed_field
    def rank_attack(self) -> int:
        return (
            PalObjects.get_byte_property(self._save_parameter["Rank_Attack"])
            if "Rank_Attack" in self._save_parameter
            else 0
        )

    @rank_attack.setter
    def rank_attack(self, value: int):
        if value == 0:
            safe_remove(self._save_parameter, "Rank_Attack")
            return
        self._save_parameter["Rank_Attack"] = PalObjects.ByteProperty(value)

    @computed_field
    def rank_defense(self) -> int:
        return (
            PalObjects.get_byte_property(self._save_parameter["Rank_Defence"])
            if "Rank_Defence" in self._save_parameter
            else 0
        )

    @rank_defense.setter
    def rank_defense(self, value: int):
        if value == 0:
            safe_remove(self._save_parameter, "Rank_Defence")
            return
        self._save_parameter["Rank_Defence"] = PalObjects.ByteProperty(value)

    @computed_field
    def rank_craftspeed(self) -> int:
        return (
            PalObjects.get_byte_property(self._save_parameter["Rank_CraftSpeed"])
            if "Rank_CraftSpeed" in self._save_parameter
            else 0
        )

    @rank_craftspeed.setter
    def rank_craftspeed(self, value: int):
        if value == 0:
            safe_remove(self._save_parameter, "Rank_CraftSpeed")
            return
        self._save_parameter["Rank_CraftSpeed"] = PalObjects.ByteProperty(value)

    @computed_field
    def talent_hp(self) -> int:
        return (
            PalObjects.get_byte_property(self._save_parameter["Talent_HP"])
            if "Talent_HP" in self._save_parameter
            else 0
        )

    @talent_hp.setter
    def talent_hp(self, value: int):
        self._save_parameter["Talent_HP"] = PalObjects.ByteProperty(value)

    @computed_field
    def talent_shot(self) -> int:
        return (
            PalObjects.get_byte_property(self._save_parameter["Talent_Shot"])
            if "Talent_Shot" in self._save_parameter
            else 0
        )

    @talent_shot.setter
    def talent_shot(self, value: int):
        self._save_parameter["Talent_Shot"] = PalObjects.ByteProperty(value)

    @computed_field
    def talent_defense(self) -> int:
        return (
            PalObjects.get_byte_property(self._save_parameter["Talent_Defense"])
            if "Talent_Defense" in self._save_parameter
            else 0
        )

    @talent_defense.setter
    def talent_defense(self, value: int):
        self._save_parameter["Talent_Defense"] = PalObjects.ByteProperty(value)

    @computed_field
    def max_hp(self) -> int:
        hp_scaling = PalObjects.get_nested(
            PAL_DATA, self.character_key, "scaling", "hp"
        )
        if not hp_scaling:
            return self.hp
        condenser_bonus = (self.rank - 1) * 0.05
        hp_iv = self.talent_hp * 0.3 / 100
        hp_soul_bonus = self.rank_hp * 0.03
        alpha_scaling = 1.2 if self.is_boss or self.is_lucky else 1
        hp = math.floor(
            500
            + (5 * self.level)
            + (hp_scaling * 0.5 * self.level * (1 + hp_iv) * alpha_scaling)
        )
        return math.floor(hp * (1 + condenser_bonus) * (1 + hp_soul_bonus)) * 1000

    @computed_field
    def storage_slot(self) -> int:
        slot_id_key = "SlotID" if "SlotID" in self._save_parameter else "SlotId"
        return (
            PalObjects.get_value(
                self._save_parameter[slot_id_key]["value"]["SlotIndex"], 0
            )
            if slot_id_key in self._save_parameter
            else 0
        )

    @storage_slot.setter
    def storage_slot(self, value: int):
        slot_id_key = "SlotID" if "SlotID" in self._save_parameter else "SlotId"
        self._save_parameter[slot_id_key] = PalObjects.PalCharacterSlotId(
            self.storage_id, value
        )

    @computed_field
    def storage_id(self) -> Optional[UUID]:
        slot_id_key = "SlotID" if "SlotID" in self._save_parameter else "SlotId"
        return (
            PalObjects.get_guid(
                self._save_parameter[slot_id_key]["value"]["ContainerId"]["value"]["ID"]
            )
            if slot_id_key in self._save_parameter
            else None
        )

    @storage_id.setter
    def storage_id(self, value: UUID):
        slot_id_key = "SlotID" if "SlotID" in self._save_parameter else "SlotId"
        self._save_parameter[slot_id_key] = PalObjects.PalCharacterSlotId(
            self.storage_id, value
        )

    @computed_field
    def learned_skills(self) -> List[str]:
        return (
            PalObjects.get_array_property(self._save_parameter["MasteredWaza"])
            if "MasteredWaza" in self._save_parameter
            else []
        )

    @learned_skills.setter
    def learned_skills(self, value: List[str]):
        if not value or len(value) == 0:
            safe_remove(self._save_parameter, "MasteredWaza")
            return
        self._save_parameter["MasteredWaza"] = PalObjects.ArrayPropertyValues(
            ArrayType.ENUM_PROPERTY, value
        )

    @computed_field
    def active_skills(self) -> List[str]:
        return (
            PalObjects.get_array_property(self._save_parameter["EquipWaza"])
            if "EquipWaza" in self._save_parameter
            else []
        )

    @active_skills.setter
    def active_skills(self, value: List[str]):
        self._save_parameter["EquipWaza"] = PalObjects.ArrayPropertyValues(
            ArrayType.ENUM_PROPERTY, value
        )

    @computed_field
    def passive_skills(self) -> List[str]:
        return (
            PalObjects.get_array_property(self._save_parameter["PassiveSkillList"])
            if "PassiveSkillList" in self._save_parameter
            else []
        )

    @passive_skills.setter
    def passive_skills(self, value: List[str]):
        self._save_parameter["PassiveSkillList"] = PalObjects.ArrayPropertyValues(
            ArrayType.NAME_PROPERTY, value
        )

    @property
    def character_save(self) -> Dict[str, Any]:
        return self._character_save

    @computed_field
    def work_suitability(self) -> Dict[WorkSuitability, int]:
        if "GotWorkSuitabilityAddRankList" not in self._save_parameter:
            return {}

        work_suitability_rank_list = PalObjects.get_array_property(
            self._save_parameter["GotWorkSuitabilityAddRankList"]
        )
        work_suitability = {}

        for work_suitability_rank in work_suitability_rank_list:
            work_suit = WorkSuitability.from_value(
                PalObjects.get_enum_property(work_suitability_rank["WorkSuitability"])
            )
            rank = PalObjects.get_value(work_suitability_rank["Rank"])
            work_suitability[work_suit] = rank
        return work_suitability

    @work_suitability.setter
    def work_suitability(self, value: Dict[WorkSuitability, int]):
        work_suitability = {k: v for k, v in value.items() if v != 0}
        safe_remove(self._save_parameter, "GotWorkSuitabilityAddRankList")
        if not work_suitability or len(work_suitability.values()) == 0:
            return
        self._save_parameter["GotWorkSuitabilityAddRankList"] = (
            PalObjects.GotWorkSuitabilityRankList(work_suitability)
        )

    @computed_field
    def is_sick(self) -> bool:
        if self._is_dps:
            return False
        return any(
            t in self._save_parameter
            for t in PAL_SICK_TYPES
            if t not in ["HungerType", "SanityValue"]
        )

    def clone(
        self, instance_id: UUID, storage_id: UUID, storage_slot: int, nickname: str
    ) -> "Pal":
        new_pal = copy.deepcopy(self)
        new_pal.instance_id = instance_id
        new_pal.nickname = nickname
        new_pal.storage_id = storage_id
        new_pal.storage_slot = storage_slot
        PalObjects.set_value(
            new_pal.character_save["key"]["PlayerUId"], value=PalObjects.EMPTY_UUID
        )
        return new_pal

    def update_from(self, other_pal: PalDTO):
        logger.debug(f"Updating pal from {other_pal}")

        type_converters = {
            "instance_id": lambda x: UUID(str(x)) if x else None,
            "owner_uid": lambda x: UUID(str(x)) if x else None,
            "group_id": lambda x: UUID(str(x)) if x else None,
            "storage_id": lambda x: UUID(str(x)) if x else None,
            "gender": lambda x: PalGender.from_value(x) if x else None,
            "stomach": float,
            "sanity": float,
            "hp": int,
            "level": int,
            "exp": int,
            "rank": int,
            "rank_hp": int,
            "rank_attack": int,
            "rank_defense": int,
            "rank_craftspeed": int,
            "talent_hp": int,
            "talent_shot": int,
            "talent_defense": int,
            "storage_slot": int,
            "is_lucky": bool,
            "is_boss": bool,
            "learned_skills": list,
            "active_skills": list,
            "passive_skills": list,
            "work_suitability": dict,
            "nickname": str,
            "filtered_nickname": str,
        }

        skip_properties = {
            "is_predator",
            "is_tower",
            "is_sick",
            "name",
            "max_hp",
            "character_key",
        }

        for key, value in other_pal.model_dump().items():
            if key in skip_properties or value is None:
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
        self.hp = self.max_hp
        if not self._is_dps:
            self.heal()

    def heal(self):
        for sick_type in PAL_SICK_TYPES:
            safe_remove(self._save_parameter, sick_type)

        self.stomach = PalObjects.get_nested(
            PAL_DATA, self.character_key, "max_full_stomach"
        )
        self.sanity = 100.0

    def _format_boss_character_id(self, is_boss: bool = False):
        has_boss_prefix = self.character_id.startswith("BOSS_")
        if has_boss_prefix != is_boss:
            self.character_id = (
                f"BOSS_{self.character_key}" if is_boss else self.character_key
            )

    def populate_status_point_lists(self):
        self._save_parameter["GotStatusPointList"] = PalObjects.GetStatusPointList(
            "GotStatusPointList", PalObjects.StatusNames
        )
        self._save_parameter["GotExStatusPointList"] = PalObjects.GetStatusPointList(
            "GotExStatusPointList", PalObjects.ExStatusNames
        )

    def remove_status_point_lists(self):
        self._save_parameter["GotStatusPointList"]["value"]["values"] = []
        self._save_parameter["GotExStatusPointList"]["value"]["values"] = []

    def reset(self):
        self.instance_id = PalObjects.EMPTY_UUID
        self.owner_uid = PalObjects.EMPTY_UUID
        self.character_id = "None"
        self.nickname = ""
        self.filtered_nickname = ""
        self.storage_id = PalObjects.EMPTY_UUID
        self.storage_slot = -1
        self.learned_skills = []
        self.active_skills = []
        self.passive_skills = []
        self.hp = 0
        self.rank = 0
        self.rank_hp = 0
        self.rank_attack = 0
        self.rank_defense = 0
        self.rank_craftspeed = 0
        self.talent_hp = 0
        self.talent_shot = 0
        self.talent_defense = 0
        self.work_suitability = {}
        self.level = 1
        self.exp = 0
        self.remove_status_point_lists()
