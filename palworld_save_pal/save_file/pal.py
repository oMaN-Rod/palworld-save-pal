import copy
from typing import Optional, Dict, Any, List
from uuid import UUID
from pydantic import BaseModel, Field, PrivateAttr


from palworld_save_pal.save_file.utils import safe_remove
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
    rank_hp: int = 0
    rank_attack: int = 0
    rank_defense: int = 0
    rank_craftspeed: int = 0
    talent_hp: int = 0
    talent_shot: int = 0
    talent_defense: int = 0
    rank: int = 1
    level: int = 1
    exp: int = 0
    nickname: Optional[str] = ""
    is_tower: bool = False
    storage_id: Optional[UUID] = None
    stomach: float = 0
    storage_slot: int = 0
    learned_skills: List[str] = Field(default_factory=list)
    active_skills: List[str] = Field(default_factory=list)
    passive_skills: List[str] = Field(default_factory=list)
    hp: int = 0
    max_hp: int = 0
    elements: Optional[List[Element]] = Field(default_factory=list)
    state: EntryState = EntryState.NONE
    group_id: Optional[UUID] = None
    sanity: float = 0.0

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
        logger.debug("Updating Pal: %s", self)
        self._update_character_id()
        self._update_nickname()
        self._update_gender()
        self._update_equip_waza()
        self._update_mastered_waza()
        self._update_passive_skills()
        self._update_group_id()
        self._update_hp()
        self._update_level()
        self._update_exp()
        self._update_ranks()
        self._update_talents()
        self._update_lucky()
        self._update_storage_info()
        self.heal()

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
        self._get_owner_uid()
        self._get_is_lucky()
        self._get_nick_name()
        self._get_group_id()
        self._process_character_id()
        self._get_gender()
        self._get_talents()
        self._get_ranks()
        self._get_level()
        self._get_exp()
        self._get_storage_info()
        self._get_skills()
        self._get_hp()
        self._get_stomach()
        self._get_sanity()
        logger.debug("Parsed PalEntity data: %s", self)

    @property
    def owner_uid(self):
        return (
            PalObjects.get_guid(self._save_parameter["OwnerPlayerUId"])
            if "OwnerPlayerUId" in self._save_parameter
            else None
        )

    def _get_owner_uid(self):
        self.owner_uid = (
            PalObjects.get_guid(self._save_parameter["OwnerPlayerUId"])
            if "OwnerPlayerUId" in self._save_parameter
            else None
        )

    def _get_is_lucky(self):
        self.is_lucky = (
            PalObjects.get_value(self._save_parameter["IsRarePal"])
            if "IsRarePal" in self._save_parameter
            else False
        )

    def _get_nick_name(self):
        self.nickname = (
            PalObjects.get_value(self._save_parameter["NickName"])
            if "NickName" in self._save_parameter
            else None
        )

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
            else PalGender.FEMALE.prefixed()
        )
        self.gender = PalGender.from_value(gender)

    def _get_stomach(self):
        self.stomach = (
            PalObjects.get_value(self._save_parameter["FullStomach"], 0)
            if "FullStomach" in self._save_parameter
            else 150
        )

    def _get_sanity(self):
        self.sanity = (
            PalObjects.get_value(self._save_parameter["SanityValue"], 100.0)
            if "SanityValue" in self._save_parameter
            else 100.0
        )

    def heal(self):
        safe_remove(self._save_parameter, "PalReviveTimer")
        safe_remove(self._save_parameter, "PhysicalHealth")
        safe_remove(self._save_parameter, "WorkerSick")
        safe_remove(self._save_parameter, "HungerType")
        safe_remove(self._save_parameter, "SanityValue")
        self.sanity = 100.0

    def _get_talents(self):
        self.talent_hp = (
            PalObjects.get_byte_property(self._save_parameter["Talent_HP"])
            if "Talent_HP" in self._save_parameter
            else 0
        )
        self.talent_shot = (
            PalObjects.get_byte_property(self._save_parameter["Talent_Shot"])
            if "Talent_Shot" in self._save_parameter
            else 0
        )
        self.talent_defense = (
            PalObjects.get_byte_property(self._save_parameter["Talent_Defense"])
            if "Talent_Defense" in self._save_parameter
            else 0
        )

    def _get_ranks(self):
        self.rank = (
            PalObjects.get_byte_property(self._save_parameter["Rank"])
            if "Rank" in self._save_parameter
            else 0
        )
        self.rank_hp = (
            PalObjects.get_byte_property(self._save_parameter["Rank_HP"])
            if "Rank_HP" in self._save_parameter
            else 0
        )
        self.rank_attack = (
            PalObjects.get_byte_property(self._save_parameter["Rank_Attack"])
            if "Rank_Attack" in self._save_parameter
            else 0
        )
        self.rank_defense = (
            PalObjects.get_byte_property(self._save_parameter["Rank_Defence"])
            if "Rank_Defence" in self._save_parameter
            else 0
        )
        self.rank_craftspeed = (
            PalObjects.get_byte_property(self._save_parameter["Rank_CraftSpeed"])
            if "Rank_CraftSpeed" in self._save_parameter
            else 0
        )

    def _get_level(self):
        self.level = (
            PalObjects.get_byte_property(self._save_parameter["Level"])
            if "Level" in self._save_parameter
            else 1
        )

    def _get_exp(self):
        self.exp = (
            PalObjects.get_value(self._save_parameter["Exp"])
            if "Exp" in self._save_parameter
            else 0
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
        self.active_skills = (
            PalObjects.get_array_property(self._save_parameter["EquipWaza"])
            if "EquipWaza" in self._save_parameter
            else []
        )

    def _get_hp(self):
        self.hp = (
            PalObjects.get_fixed_point64(self._save_parameter["Hp"])
            if "Hp" in self._save_parameter
            else 0
        )

    def _get_group_id(self):
        self.group_id = PalObjects.as_uuid(
            PalObjects.get_nested(
                self._character_save, "value", "RawData", "value", "group_id"
            )
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
        active_skills = self.active_skills if self.active_skills else []

        if "EquipWaza" in self._save_parameter:
            PalObjects.set_array_property(
                self._save_parameter["EquipWaza"], values=active_skills
            )
        else:
            self._save_parameter["EquipWaza"] = PalObjects.ArrayPropertyValues(
                ArrayType.ENUM_PROPERTY, active_skills
            )

    def _update_mastered_waza(self):
        if not self.learned_skills or len(self.learned_skills) == 0:
            safe_remove(self._save_parameter, "MasteredWaza")
            return

        if "MasteredWaza" in self._save_parameter:
            PalObjects.set_array_property(
                self._save_parameter["MasteredWaza"], values=self.learned_skills
            )
        else:
            self._save_parameter["MasteredWaza"] = PalObjects.ArrayPropertyValues(
                ArrayType.ENUM_PROPERTY, self.learned_skills
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

    def _update_group_id(self) -> None:
        if "group_id" in self._character_save["value"]["RawData"]["value"]:
            PalObjects.set_nested(
                self._character_save,
                "value",
                "RawData",
                "value",
                "group_id",
                value=self.group_id,
            )

    def _update_slot_idx(self, slot_idx: int) -> None:
        PalObjects.set_value(
            PalObjects.get_nested(self._save_parameter, "SlotID", "value", "SlotIndex"),
            value=slot_idx,
        )

    def _update_hp(self) -> None:
        if "Hp" in self._save_parameter:
            PalObjects.set_fixed_point64(self._save_parameter["Hp"], value=self.hp)
        else:
            self._save_parameter["Hp"] = PalObjects.FixedPoint64(self.hp)

    def _update_level(self) -> None:
        if self.level <= 1:
            safe_remove(self._save_parameter, "Level")
            return

        if "Level" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Level"], value=self.level
            )
        else:
            self._save_parameter["Level"] = PalObjects.ByteProperty(self.level)

    def _update_exp(self):
        if self.exp == 0:
            safe_remove(self._save_parameter, "Exp")
            return

        if "Exp" in self._save_parameter:
            PalObjects.set_value(self._save_parameter["Exp"], value=self.exp)
        elif self.exp > 0:
            self._save_parameter["Exp"] = PalObjects.Int64Property(self.exp)

    def _update_ranks(self) -> None:
        self._update_rank()
        self._update_rank_hp()
        self._update_rank_attack()
        self._update_rank_defense()
        self._update_rank_craftspeed()

    def _update_rank(self) -> None:
        if self.rank + 1 <= 1:
            safe_remove(self._save_parameter, "Rank")
            return

        if "Rank" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Rank"], value=self.rank + 1
            )
        else:
            self._save_parameter["Rank"] = PalObjects.ByteProperty(self.rank + 1)

    def _update_rank_hp(self) -> None:
        if self.rank_hp == 0:
            safe_remove(self._save_parameter, "Rank_HP")
            return

        if "Rank_HP" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Rank_HP"], value=self.rank_hp
            )
        else:
            self._save_parameter["Rank_HP"] = PalObjects.ByteProperty(self.rank_hp)

    def _update_rank_attack(self) -> None:
        if self.rank_attack == 0:
            safe_remove(self._save_parameter, "Rank_Attack")
            return

        if "Rank_Attack" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Rank_Attack"], value=self.rank_attack
            )
        else:
            self._save_parameter["Rank_Attack"] = PalObjects.ByteProperty(
                self.rank_attack
            )

    def _update_rank_defense(self) -> None:
        if self.rank_defense == 0:
            safe_remove(self._save_parameter, "Rank_Defence")
            return

        if "Rank_Defence" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Rank_Defence"], value=self.rank_defense
            )
        else:
            self._save_parameter["Rank_Defence"] = PalObjects.ByteProperty(
                self.rank_defense
            )

    def _update_rank_craftspeed(self) -> None:
        if self.rank_craftspeed == 0:
            safe_remove(self._save_parameter, "Rank_CraftSpeed")
            return

        if "Rank_CraftSpeed" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Rank_CraftSpeed"], value=self.rank_craftspeed
            )
        else:
            self._save_parameter["Rank_CraftSpeed"] = PalObjects.ByteProperty(
                self.rank_craftspeed
            )

    def _update_talents(self) -> None:
        self._update_talent_hp()
        self._update_talent_shot()
        self._update_talent_defense()

    def _update_talent_hp(self) -> None:
        if "Talent_HP" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Talent_HP"], value=self.talent_hp
            )
        else:
            self._save_parameter["Talent_HP"] = PalObjects.ByteProperty(self.talent_hp)

    def _update_talent_shot(self) -> None:
        if "Talent_Shot" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Talent_Shot"], value=self.talent_shot
            )
        else:
            self._save_parameter["Talent_Shot"] = PalObjects.ByteProperty(
                self.talent_shot
            )

    def _update_talent_defense(self) -> None:
        if "Talent_Defense" in self._save_parameter:
            PalObjects.set_byte_property(
                self._save_parameter["Talent_Defense"], value=self.talent_defense
            )
        else:
            self._save_parameter["Talent_Defense"] = PalObjects.ByteProperty(
                self.talent_defense
            )

    def _update_character_id(self) -> None:
        character_id = (
            f"BOSS_{self.character_id}"
            if self.is_boss or self.is_lucky
            else self.character_id
        )
        PalObjects.set_value(self._save_parameter["CharacterID"], value=character_id)

    def _update_lucky(self) -> None:
        if not self.is_lucky:
            safe_remove(self._save_parameter, "IsRarePal")
            return

        if "IsRarePal" in self._save_parameter:
            PalObjects.set_value(self._save_parameter["IsRarePal"], value=self.is_lucky)
        else:
            self._save_parameter["IsRarePal"] = PalObjects.BoolProperty(self.is_lucky)

    def _update_storage_info(self) -> None:
        if "SlotID" in self._save_parameter:
            slot_id = PalObjects.get_value(self._save_parameter["SlotID"])
            PalObjects.set_value(
                PalObjects.get_nested(slot_id, "ContainerId", "value", "ID"),
                value=self.storage_id,
            )
            PalObjects.set_value(slot_id["SlotIndex"], value=self.storage_slot)
        else:
            self._save_parameter["SlotID"] = PalObjects.PalCharacterSlotId(
                self.storage_id, self.storage_slot
            )
