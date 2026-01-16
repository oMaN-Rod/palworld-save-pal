from typing import Dict, List, Optional
from uuid import uuid4
from pydantic import ConfigDict
from sqlmodel import SQLModel, Field, Relationship
from sqlalchemy import Column, JSON, Enum, String, Boolean, Integer, Float

from palworld_save_pal.dto.item_container_slot import ItemContainerSlotDTO
from palworld_save_pal.game.pal_objects import PalGender


class StorageContainerPreset(SQLModel):
    key: str
    slots: List[ItemContainerSlotDTO]


class PalPreset(SQLModel, table=True):
    id: str = Field(default_factory=lambda: str(uuid4()), primary_key=True)
    lock: bool = Field(sa_column=Column(Boolean, nullable=False))
    lock_element: bool = Field(sa_column=Column(Boolean, nullable=False), default=False)
    element: str | None = Field(default=None, sa_column=Column(String))
    character_id: str | None = Field(default=None, sa_column=Column(String))
    is_lucky: bool | None = Field(default=None, sa_column=Column(Boolean))
    is_boss: bool | None = Field(default=None, sa_column=Column(Boolean))
    gender: PalGender | None = Field(default=None, sa_column=Column(Enum(PalGender)))
    rank_hp: int | None = Field(default=None, sa_column=Column(Integer))
    rank_attack: int | None = Field(default=None, sa_column=Column(Integer))
    rank_defense: int | None = Field(default=None, sa_column=Column(Integer))
    rank_craftspeed: int | None = Field(default=None, sa_column=Column(Integer))
    talent_hp: int | None = Field(default=None, sa_column=Column(Integer))
    talent_shot: int | None = Field(default=None, sa_column=Column(Integer))
    talent_defense: int | None = Field(default=None, sa_column=Column(Integer))
    rank: int | None = Field(default=None, sa_column=Column(Integer))
    level: int | None = Field(default=None, sa_column=Column(Integer))
    exp: int | None = Field(default=None, sa_column=Column(Integer))
    learned_skills: List[str] | None = Field(default=None, sa_column=Column(JSON))
    active_skills: List[str] | None = Field(default=None, sa_column=Column(JSON))
    passive_skills: List[str] | None = Field(default=None, sa_column=Column(JSON))
    sanity: float | None = Field(default=None, sa_column=Column(Float))
    work_suitability: Dict[str, int] | None = Field(
        default=None, sa_column=Column(JSON)
    )
    # Added in v0.15.0
    nickname: str | None = Field(default=None, sa_column=Column(String))
    filtered_nickname: str | None = Field(default=None, sa_column=Column(String))
    stomach: float | None = Field(default=None, sa_column=Column(Float))
    hp: int | None = Field(default=None, sa_column=Column(Integer))
    friendship_point: int | None = Field(default=None, sa_column=Column(Integer))

    model_config = ConfigDict(arbitrary_types_allowed=True)

    preset_profile: "PresetProfile" | None = Relationship(
        back_populates="pal_preset",
        sa_relationship_kwargs={"foreign_keys": "PresetProfile.pal_preset_id"},
    )


class PresetProfile(SQLModel, table=True):
    id: str = Field(default_factory=lambda: str(uuid4()), primary_key=True)
    name: str = Field(sa_column=Column(String, nullable=False))
    type: str = Field(sa_column=Column(String, nullable=False))
    skills: List[str] | None = Field(default=None, sa_column=Column(JSON))

    common_container: List[dict] | None = Field(default=None, sa_column=Column(JSON))
    essential_container: List[dict] | None = Field(default=None, sa_column=Column(JSON))
    weapon_load_out_container: List[dict] | None = Field(
        default=None, sa_column=Column(JSON)
    )
    player_equipment_armor_container: List[dict] | None = Field(
        default=None, sa_column=Column(JSON)
    )
    food_equip_container: List[dict] | None = Field(
        default=None, sa_column=Column(JSON)
    )
    storage_container: dict | None = Field(default=None, sa_column=Column(JSON))

    pal_preset_id: str | None = Field(default=None, foreign_key="palpreset.id")
    pal_preset: PalPreset | None = Relationship(back_populates="preset_profile")

    model_config = ConfigDict(arbitrary_types_allowed=True)
