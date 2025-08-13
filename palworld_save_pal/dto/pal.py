from typing import Any, Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel, computed_field

from palworld_save_pal.game.pal_objects import PalGender, WorkSuitability
from palworld_save_pal.game.utils import format_character_key
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class PalDTO(BaseModel):
    instance_id: UUID
    owner_uid: Optional[UUID]
    character_id: str
    is_lucky: Optional[bool]
    is_boss: Optional[bool]
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
    work_suitability: Dict[WorkSuitability, int]
    is_sick: bool
    friendship_point: int

    @computed_field
    def character_key(self) -> str:
        return format_character_key(self.character_id)

    @classmethod
    def from_dict(cls, other_pal: Dict[str, Any]) -> "PalDTO":
        logger.debug(f"Creating PalDTO from dict: {other_pal}")

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
            "learned_skills": list,
            "active_skills": list,
            "passive_skills": list,
            "work_suitability": dict,
            "nickname": str,
            "filtered_nickname": str,
            "friendship_point": int,
        }

        field_values = {}
        model_fields = cls.model_fields.keys()

        for key, value in other_pal.items():
            if key in model_fields:
                try:
                    if key in type_converters:
                        converted_value = type_converters[key](value)
                        field_values[key] = converted_value
                    else:
                        field_values[key] = value
                except Exception as e:
                    logger.warning(f"Failed to convert {key}: {str(e)}")
                    continue

        defaults = {
            "instance_id": UUID("00000000-0000-0000-0000-000000000000"),
            "character_id": other_pal.get("character_id", ""),
            "gender": PalGender.MALE,
            "rank_hp": 0,
            "rank_attack": 0,
            "rank_defense": 0,
            "rank_craftspeed": 0,
            "talent_hp": 0,
            "talent_shot": 0,
            "talent_defense": 0,
            "rank": 0,
            "level": 1,
            "exp": 0,
            "is_tower": False,
            "storage_id": UUID("00000000-0000-0000-0000-000000000000"),
            "stomach": 0.0,
            "storage_slot": 0,
            "learned_skills": [],
            "active_skills": [],
            "passive_skills": [],
            "hp": 1,
            "max_hp": 1,
            "sanity": 1.0,
            "work_suitability": {},
            "is_sick": False,
            "friendship_point": 0,
        }

        for field_name in model_fields:
            if field_name not in field_values and field_name in defaults:
                field_values[field_name] = defaults[field_name]

        try:
            return cls(**field_values)
        except Exception as e:
            logger.error(f"Failed to create PalDTO: {str(e)}")
            logger.debug(f"Field values: {field_values}")
            raise
