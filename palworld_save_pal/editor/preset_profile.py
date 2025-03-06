from typing import Dict, List, Optional
from uuid import uuid4, UUID
from pydantic import ConfigDict
from sqlmodel import SQLModel, Field, Relationship
from sqlalchemy import Column, JSON, Enum, String, Boolean, Integer, Float

from palworld_save_pal.game.item_container_slot import ItemContainerSlot
from palworld_save_pal.game.pal_objects import PalGender, WorkSuitability


class StorageContainerPreset(SQLModel):
    key: str
    slots: List[ItemContainerSlot]


class PalPreset(SQLModel, table=True):
    id: UUID = Field(default_factory=uuid4, primary_key=True)
    lock: bool = Field(sa_column=Column(Boolean, nullable=False))
    character_id: Optional[str] = Field(default=None, sa_column=Column(String))
    is_lucky: Optional[bool] = Field(default=None, sa_column=Column(Boolean))
    is_boss: Optional[bool] = Field(default=None, sa_column=Column(Boolean))
    gender: Optional[PalGender] = Field(default=None, sa_column=Column(Enum(PalGender)))
    rank_hp: Optional[int] = Field(default=None, sa_column=Column(Integer))
    rank_attack: Optional[int] = Field(default=None, sa_column=Column(Integer))
    rank_defense: Optional[int] = Field(default=None, sa_column=Column(Integer))
    rank_craftspeed: Optional[int] = Field(default=None, sa_column=Column(Integer))
    talent_hp: Optional[int] = Field(default=None, sa_column=Column(Integer))
    talent_shot: Optional[int] = Field(default=None, sa_column=Column(Integer))
    talent_defense: Optional[int] = Field(default=None, sa_column=Column(Integer))
    rank: Optional[int] = Field(default=None, sa_column=Column(Integer))
    level: Optional[int] = Field(default=None, sa_column=Column(Integer))
    exp: Optional[int] = Field(default=None, sa_column=Column(Integer))
    learned_skills: Optional[List[str]] = Field(default=None, sa_column=Column(JSON))
    active_skills: Optional[List[str]] = Field(default=None, sa_column=Column(JSON))
    passive_skills: Optional[List[str]] = Field(default=None, sa_column=Column(JSON))
    sanity: Optional[float] = Field(default=None, sa_column=Column(Float))
    work_suitability: Optional[Dict[str, int]] = Field(
        default=None, sa_column=Column(JSON)
    )

    model_config = ConfigDict(arbitrary_types_allowed=True)

    # preset_profile: Optional["PresetProfile"] = Relationship(
    #     back_populates="pal_preset"
    # )


class PresetProfile(SQLModel, table=True):
    id: UUID = Field(default_factory=uuid4, primary_key=True)
    name: str = Field(sa_column=Column(String, nullable=False))
    type: str = Field(sa_column=Column(String, nullable=False))
    skills: Optional[List[str]] = Field(default=None, sa_column=Column(JSON))

    common_container: Optional[List[dict]] = Field(default=None, sa_column=Column(JSON))
    essential_container: Optional[List[dict]] = Field(
        default=None, sa_column=Column(JSON)
    )
    weapon_load_out_container: Optional[List[dict]] = Field(
        default=None, sa_column=Column(JSON)
    )
    player_equipment_armor_container: Optional[List[dict]] = Field(
        default=None, sa_column=Column(JSON)
    )
    food_equip_container: Optional[List[dict]] = Field(
        default=None, sa_column=Column(JSON)
    )
    storage_container: Optional[dict] = Field(default=None, sa_column=Column(JSON))

    pal_preset_id: Optional[int] = Field(default=None, sa_column=Column(Integer))
    # pal_preset: Optional[PalPreset] = Relationship(back_populates="preset_profile")

    model_config = ConfigDict(arbitrary_types_allowed=True)
