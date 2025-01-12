import copy
import math
from typing import Optional, Dict, Any, List
from uuid import UUID
from pydantic import BaseModel, PrivateAttr, computed_field


from palworld_save_pal.utils.dict import safe_remove
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.game.pal_objects import *


logger = create_logger(__name__)
PAL_DATA = JsonManager("data/json/pals.json").read()


class PalDTO(BaseModel):
    instance_id: UUID
    owner_uid: UUID
    character_id: str
    is_lucky: bool
    is_boss: bool
    gender: PalGender
    rank_hp: int
    rank_attack: int
    rank_defense: int
    rank_craftspeed: int
    talent_hp: int
    talent_shot: int
    talent_defense: int
    rank: int
    level: int
    exp: int
    nickname: Optional[str]
    is_tower: bool
    storage_id: UUID
    stomach: float
    storage_slot: int
    learned_skills: List[str]
    active_skills: List[str]
    passive_skills: List[str]
    hp: int
    max_hp: int
    group_id: Optional[UUID]
    sanity: float


class Pal(BaseModel):
    _instance_id: Optional[UUID] = None
    _character_id: Optional[str] = None
    _character_key: Optional[str] = None
    _owner_uid: Optional[UUID] = None
    _is_lucky: bool = False
    _is_predator: bool = False
    _is_boss: bool = False
    _is_tower: bool = False
    _gender: Optional[PalGender] = None
    _nickname: Optional[str] = ""
    _stomach: float = 0
    _sanity: float = 0.0
    _hp: int = 0
    _max_hp: int = 0
    _level: int = 1
    _exp: int = 0
    _group_id: Optional[UUID] = None
    _rank: int = 0
    _rank_hp: int = 0
    _rank_attack: int = 0
    _rank_defense: int = 0
    _rank_craftspeed: int = 0
    _talent_hp: int = 0
    _talent_shot: int = 0
    _talent_defense: int = 0
    _storage_slot: int = 0
    _storage_id: Optional[UUID] = None
    _learned_skills: List[str] = []
    _active_skills: List[str] = []
    _passive_skills: List[str] = []

    _character_save: Dict[str, Any] = PrivateAttr(default_factory=dict)
    _save_parameter: Dict[str, Any] = PrivateAttr(default_factory=dict)

    def __init__(self, data=None, **kwargs):
        if data is not None:
            super().__init__()
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
        else:
            super().__init__(**kwargs)

    @computed_field
    def instance_id(self) -> UUID:
        self._instance_id = PalObjects.get_guid(
            self._character_save["key"]["InstanceId"]
        )
        return self._instance_id

    @instance_id.setter
    def instance_id(self, value: UUID):
        self._instance_id = value
        PalObjects.set_value(
            self._character_save["key"]["InstanceId"], value=self._instance_id
        )

    @computed_field
    def character_id(self) -> Optional[str]:
        self._character_id = (
            PalObjects.get_value(self._save_parameter["CharacterID"])
            if "CharacterID" in self._save_parameter
            else None
        )
        return self._character_id

    @character_id.setter
    def character_id(self, value: str):
        self._character_id = value
        PalObjects.set_value(
            self._save_parameter["CharacterID"], value=self._character_id
        )

    @computed_field
    def character_key(self) -> Optional[str]:
        if self.character_id.lower().startswith("boss_"):
            self._character_key = self.character_id[5:]
        elif self.character_id.lower().startswith("predator_"):
            self._character_key = self.character_id[9:]
        else:
            self._character_key = self.character_id
        if self._character_key.lower() == "sheepball":
            self._character_key = "Sheepball"
        return self._character_key

    @computed_field
    def owner_uid(self) -> Optional[UUID]:
        self._owner_uid = (
            PalObjects.get_guid(self._save_parameter["OwnerPlayerUId"])
            if "OwnerPlayerUId" in self._save_parameter
            else None
        )
        return self._owner_uid

    @owner_uid.setter
    def owner_uid(self, value: UUID):
        self._owner_uid = value
        PalObjects.set_value(
            self._save_parameter["OwnerPlayerUId"], value=self._owner_uid
        )

    @computed_field
    def is_lucky(self) -> bool:
        self._is_lucky = (
            PalObjects.get_value(self._save_parameter["IsRarePal"])
            if "IsRarePal" in self._save_parameter
            else False
        )
        return self._is_lucky

    @is_lucky.setter
    def is_lucky(self, value: bool):
        self._is_lucky = value
        if self._is_lucky:
            if "IsRarePal" in self._save_parameter:
                PalObjects.set_value(
                    self._save_parameter["IsRarePal"], value=self._is_lucky
                )
            else:
                self._save_parameter["IsRarePal"] = PalObjects.BoolProperty(
                    self._is_lucky
                )
        else:
            safe_remove(self._save_parameter, "IsRarePal")

    @computed_field
    def is_boss(self) -> bool:
        self._is_boss = False
        if self.character_id.startswith("BOSS_") and not self.is_lucky:
            self._is_boss = True
        return self._is_boss

    @is_boss.setter
    def is_boss(self, value: bool):
        self._is_boss = value
        if self._is_boss:
            if not self.character_id.startswith("BOSS_"):
                self.character_id = f"BOSS_{self.character_key}"
            if self.is_lucky:
                self.is_lucky = False

        logger.debug(f"Setting is_boss to {self._is_boss} {self._character_id}")

    @computed_field
    def is_predator(self) -> bool:
        self._is_predator = (
            self.character_id.startswith("PREDATOR_") if self.character_id else False
        )
        return self._is_predator

    @computed_field
    def is_tower(self) -> bool:
        self._is_tower = (
            self.character_id.startswith("GYM_") if self.character_id else False
        )
        return self._is_tower

    @computed_field
    def gender(self) -> Optional[PalGender]:
        g = (
            PalObjects.get_enum_property(self._save_parameter["Gender"])
            if "Gender" in self._save_parameter
            else PalGender.FEMALE.prefixed()
        )
        self._gender = PalGender.from_value(g)
        return self._gender

    @gender.setter
    def gender(self, value: PalGender):
        self._gender = value
        if "Gender" in self._save_parameter:
            PalObjects.set_enum_property(
                self._save_parameter["Gender"], value=self._gender.prefixed()
            )
        else:
            self._save_parameter["Gender"] = PalObjects.EnumProperty(
                "EPalGenderType", self._gender.prefixed()
            )

    @computed_field
    def nickname(self) -> Optional[str]:
        self._nickname = (
            PalObjects.get_value(self._save_parameter["NickName"])
            if "NickName" in self._save_parameter
            else None
        )
        return self._nickname

    @nickname.setter
    def nickname(self, value: str):
        self._nickname = value
        if "NickName" in self._save_parameter:
            PalObjects.set_value(self._save_parameter["NickName"], value=self._nickname)
        else:
            self._save_parameter["NickName"] = PalObjects.StrProperty(self._nickname)

    @computed_field
    def group_id(self) -> Optional[UUID]:
        self._group_id = PalObjects.as_uuid(
            PalObjects.get_nested(
                self._character_save, "value", "RawData", "value", "group_id"
            )
        )
        return self._group_id

    @group_id.setter
    def group_id(self, value: UUID):
        self._group_id = value
        if "group_id" in self._character_save["value"]["RawData"]["value"]:
            PalObjects.set_nested(
                self._character_save,
                "value",
                "RawData",
                "value",
                "group_id",
                value=self.group_id,
            )

    @computed_field
    def stomach(self) -> float:
        self._stomach = (
            PalObjects.get_value(self._save_parameter["FullStomach"], 150)
            if "FullStomach" in self._save_parameter
            else 150
        )
        return self._stomach

    @stomach.setter
    def stomach(self, value: float):
        self._stomach = value
        if "FullStomach" in self._save_parameter:
            PalObjects.set_value(
                self._save_parameter["FullStomach"], value=self._stomach
            )
        else:
            self._save_parameter["FullStomach"] = PalObjects.FloatProperty(
                self._stomach
            )

    @computed_field
    def sanity(self) -> float:
        self._sanity = (
            PalObjects.get_value(self._save_parameter["SanityValue"], 100.0)
            if "SanityValue" in self._save_parameter
            else 100.0
        )
        return self._sanity

    @sanity.setter
    def sanity(self, value: float):
        self._sanity = value
        if "SanityValue" in self._save_parameter:
            PalObjects.set_value(
                self._save_parameter["SanityValue"], value=self._sanity
            )
        else:
            self._save_parameter["SanityValue"] = PalObjects.FloatProperty(self._sanity)

    @computed_field
    def hp(self) -> int:
        if "HP" in self._save_parameter:
            self._save_parameter["Hp"] = self._save_parameter.pop("HP")
        self._hp = (
            PalObjects.get_fixed_point64(self._save_parameter["Hp"])
            if "Hp" in self._save_parameter
            else 0
        )
        return self._hp

    @hp.setter
    def hp(self, value: int):
        self._hp = value
        if "Hp" in self._save_parameter:
            PalObjects.set_fixed_point64(self._save_parameter["Hp"], value=self._hp)
        else:
            self._save_parameter["Hp"] = PalObjects.FixedPoint64(self._hp)

    @computed_field
    def level(self) -> int:
        self._level = (
            PalObjects.get_byte_property(self._save_parameter["Level"])
            if "Level" in self._save_parameter
            else 1
        )
        return self._level

    @level.setter
    def level(self, value: int):
        self._level = value
        if self._level <= 1:
            safe_remove(self._save_parameter, "Level")
            return
        if "Level" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Level"], value=self._level
            )
        else:
            self._save_parameter["Level"] = PalObjects.ByteProperty(self._level)

    @computed_field
    def exp(self) -> int:
        self._exp = (
            PalObjects.get_value(self._save_parameter["Exp"])
            if "Exp" in self._save_parameter
            else 0
        )
        return self._exp

    @exp.setter
    def exp(self, value: int):
        self._exp = value
        if self._exp == 0:
            safe_remove(self._save_parameter, "Exp")
            return
        if "Exp" in self._save_parameter:
            PalObjects.set_value(self._save_parameter["Exp"], value=self._exp)
        else:
            self._save_parameter["Exp"] = PalObjects.Int64Property(self._exp)

    @computed_field
    def rank(self) -> int:
        self._rank = (
            int(PalObjects.get_byte_property(self._save_parameter["Rank"])) - 1
            if "Rank" in self._save_parameter
            else 0
        )
        return self._rank

    @rank.setter
    def rank(self, value: int):
        if value != self._rank:
            self._rank = min(value + 1, 5)
        if self._rank == 0:
            safe_remove(self._save_parameter, "Rank")
            return

        if "Rank" in self._save_parameter:
            PalObjects.set_byte_property(self._save_parameter["Rank"], value=self._rank)
        else:
            self._save_parameter["Rank"] = PalObjects.ByteProperty(self._rank)

    @computed_field
    def rank_hp(self) -> int:
        self._rank_hp = (
            PalObjects.get_byte_property(self._save_parameter["Rank_HP"])
            if "Rank_HP" in self._save_parameter
            else 0
        )
        return self._rank_hp

    @rank_hp.setter
    def rank_hp(self, value: int):
        self._rank_hp = value
        if self._rank_hp == 0:
            safe_remove(self._save_parameter, "Rank_HP")
            return

        if "Rank_HP" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Rank_HP"], value=self._rank_hp
            )
        else:
            self._save_parameter["Rank_HP"] = PalObjects.ByteProperty(self._rank_hp)

    @computed_field
    def rank_attack(self) -> int:
        self._rank_attack = (
            PalObjects.get_byte_property(self._save_parameter["Rank_Attack"])
            if "Rank_Attack" in self._save_parameter
            else 0
        )
        return self._rank_attack

    @rank_attack.setter
    def rank_attack(self, value: int):
        self._rank_attack = value
        if self._rank_attack == 0:
            safe_remove(self._save_parameter, "Rank_Attack")
            return

        if "Rank_Attack" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Rank_Attack"], value=self._rank_attack
            )
        else:
            self._save_parameter["Rank_Attack"] = PalObjects.ByteProperty(
                self._rank_attack
            )

    @computed_field
    def rank_defense(self) -> int:
        self._rank_defense = (
            PalObjects.get_byte_property(self._save_parameter["Rank_Defence"])
            if "Rank_Defence" in self._save_parameter
            else 0
        )
        return self._rank_defense

    @rank_defense.setter
    def rank_defense(self, value: int):
        self._rank_defense = value
        if self._rank_defense == 0:
            safe_remove(self._save_parameter, "Rank_Defence")
            return

        if "Rank_Defence" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Rank_Defence"], value=self._rank_defense
            )
        else:
            self._save_parameter["Rank_Defence"] = PalObjects.ByteProperty(
                self._rank_defense
            )

    @computed_field
    def rank_craftspeed(self) -> int:
        self._rank_craftspeed = (
            PalObjects.get_byte_property(self._save_parameter["Rank_CraftSpeed"])
            if "Rank_CraftSpeed" in self._save_parameter
            else 0
        )
        return self._rank_craftspeed

    @rank_craftspeed.setter
    def rank_craftspeed(self, value: int):
        self._rank_craftspeed = value
        if self._rank_craftspeed == 0:
            safe_remove(self._save_parameter, "Rank_CraftSpeed")
            return

        if "Rank_CraftSpeed" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Rank_CraftSpeed"], value=self._rank_craftspeed
            )
        else:
            self._save_parameter["Rank_CraftSpeed"] = PalObjects.ByteProperty(
                self._rank_craftspeed
            )

    @computed_field
    def talent_hp(self) -> int:
        self._talent_hp = (
            PalObjects.get_byte_property(self._save_parameter["Talent_HP"])
            if "Talent_HP" in self._save_parameter
            else 0
        )
        return self._talent_hp

    @talent_hp.setter
    def talent_hp(self, value: int):
        self._talent_hp = value
        if "Talent_HP" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Talent_HP"], value=self._talent_hp
            )
        else:
            self._save_parameter["Talent_HP"] = PalObjects.ByteProperty(self._talent_hp)

    @computed_field
    def talent_shot(self) -> int:
        self._talent_shot = (
            PalObjects.get_byte_property(self._save_parameter["Talent_Shot"])
            if "Talent_Shot" in self._save_parameter
            else 0
        )
        return self._talent_shot

    @talent_shot.setter
    def talent_shot(self, value: int):
        self._talent_shot = value
        if "Talent_Shot" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Talent_Shot"], value=self._talent_shot
            )
        else:
            self._save_parameter["Talent_Shot"] = PalObjects.ByteProperty(
                self._talent_shot
            )

    @computed_field
    def talent_defense(self) -> int:
        self._talent_defense = (
            PalObjects.get_byte_property(self._save_parameter["Talent_Defense"])
            if "Talent_Defense" in self._save_parameter
            else 0
        )
        return self._talent_defense

    @talent_defense.setter
    def talent_defense(self, value: int):
        self._talent_defense = value
        if "Talent_Defense" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Talent_Defense"], value=self._talent_defense
            )
        else:
            self._save_parameter["Talent_Defense"] = PalObjects.ByteProperty(
                self._talent_defense
            )

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
        alpha_scaling = 1.2 if self.is_boss else 1
        hp = math.floor(
            500 + 5 * self.level + hp_scaling * 0.5 * self.level * (1 + hp_iv)
        )
        return (
            math.floor(hp * (1 + condenser_bonus) * (1 + hp_soul_bonus) * alpha_scaling)
            * 1000
        )

    @computed_field
    def storage_slot(self) -> int:
        self._storage_slot = (
            PalObjects.get_value(
                self._save_parameter["SlotID"]["value"]["SlotIndex"], 0
            )
            if "SlotID" in self._save_parameter
            else 0
        )
        return self._storage_slot

    @storage_slot.setter
    def storage_slot(self, value: int):
        self._storage_slot = value
        if "SlotID" in self._save_parameter:
            PalObjects.set_value(
                self._save_parameter["SlotID"]["value"]["SlotIndex"],
                value=self._storage_slot,
            )
        else:
            self._save_parameter["SlotID"] = PalObjects.PalCharacterSlotId(
                self._storage_id, self._storage_slot
            )

    @computed_field
    def storage_id(self) -> Optional[UUID]:
        self._storage_id = (
            PalObjects.get_guid(
                self._save_parameter["SlotID"]["value"]["ContainerId"]["value"]["ID"]
            )
            if "SlotID" in self._save_parameter
            else None
        )
        return self._storage_id

    @storage_id.setter
    def storage_id(self, value: UUID):
        self._storage_id = value
        if "SlotID" in self._save_parameter:
            PalObjects.set_value(
                self._save_parameter["SlotID"]["value"]["ContainerId"]["value"]["ID"],
                value=self._storage_id,
            )
        else:
            self._save_parameter["SlotID"] = PalObjects.PalCharacterSlotId(
                self._storage_id, self._storage_slot
            )

    @computed_field
    def learned_skills(self) -> List[str]:
        self._learned_skills = (
            PalObjects.get_array_property(self._save_parameter["MasteredWaza"])
            if "MasteredWaza" in self._save_parameter
            else []
        )
        return self._learned_skills

    @learned_skills.setter
    def learned_skills(self, value: List[str]):
        self._learned_skills = value
        if not value or len(value) == 0:
            safe_remove(self._save_parameter, "MasteredWaza")
            return
        if "MasteredWaza" in self._save_parameter:
            PalObjects.set_array_property(
                self._save_parameter["MasteredWaza"], values=self._learned_skills
            )
        else:
            self._save_parameter["MasteredWaza"] = PalObjects.ArrayPropertyValues(
                ArrayType.ENUM_PROPERTY, self._learned_skills
            )

    @computed_field
    def active_skills(self) -> List[str]:
        self._active_skills = (
            PalObjects.get_array_property(self._save_parameter["EquipWaza"])
            if "EquipWaza" in self._save_parameter
            else []
        )
        return self._active_skills

    @active_skills.setter
    def active_skills(self, value: List[str]):
        self._active_skills = value
        if "EquipWaza" in self._save_parameter:
            PalObjects.set_array_property(
                self._save_parameter["EquipWaza"], values=self._active_skills
            )
        else:
            self._save_parameter["EquipWaza"] = PalObjects.ArrayPropertyValues(
                ArrayType.ENUM_PROPERTY, self._active_skills
            )

    @computed_field
    def passive_skills(self) -> List[str]:
        self._passive_skills = (
            PalObjects.get_array_property(self._save_parameter["PassiveSkillList"])
            if "PassiveSkillList" in self._save_parameter
            else []
        )
        return self._passive_skills

    @passive_skills.setter
    def passive_skills(self, value: List[str]):
        self._passive_skills = value
        if "PassiveSkillList" in self._save_parameter:
            PalObjects.set_array_property(
                self._save_parameter["PassiveSkillList"], values=self._passive_skills
            )
        else:
            self._save_parameter["PassiveSkillList"] = PalObjects.ArrayPropertyValues(
                ArrayType.NAME_PROPERTY, self._passive_skills
            )

    @property
    def character_save(self) -> Dict[str, Any]:
        return self._character_save

    def clone(
        self, instance_id: UUID, storage_id: UUID, storage_slot: int, nickname: str
    ) -> "Pal":
        new_pal = copy.deepcopy(self)
        new_pal.instance_id = instance_id
        new_pal.nickname = nickname
        new_pal.storage_id = storage_id
        new_pal.storage_slot = storage_slot
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
        }

        skip_properties = {
            "is_predator",
            "is_tower",
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

        self.heal()

    def heal(self):
        safe_remove(self._save_parameter, "PalReviveTimer")
        safe_remove(self._save_parameter, "PhysicalHealth")
        safe_remove(self._save_parameter, "WorkerSick")
        safe_remove(self._save_parameter, "HungerType")
        safe_remove(self._save_parameter, "SanityValue")
        self.sanity = 100.0
