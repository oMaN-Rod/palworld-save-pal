from enum import Enum
from typing import Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel, Field


def custom_uuid_encoder(uuid: UUID) -> str:
    return str(uuid)


class Element(str, Enum):
    FIRE = "Fire"
    WATER = "Water"
    GROUND = "Ground"
    ICE = "Ice"
    NEUTRAL = "Neutral"
    DARK = "Dark"
    GRASS = "Grass"
    DRAGON = "Dragon"
    ELECTRIC = "Electric"


class PalGender(str, Enum):
    MALE = "male"
    FEMALE = "female"
    UNKNOWN = "Unknown"


class PalSummary(BaseModel):
    instance_id: UUID = Field(..., json_encoder=custom_uuid_encoder)
    character_id: str
    owner_uid: UUID
    nickname: Optional[str]
    level: int
    elements: List[str]


class Player(BaseModel):
    uid: UUID = Field(..., json_encoder=custom_uuid_encoder)
    nickname: str
    level: int
    pals: Dict[UUID, PalSummary] = Field(default_factory=dict)


class WorkSuitability(str, Enum):
    EMIT_FLAME = "EmitFlame"
    WATERING = "Watering"
    SEEDING = "Seeding"
    GENERATE_ELECTRICITY = "GenerateElectricity"
    HANDCRAFT = "Handcraft"
    COLLECTION = "Collection"
    DEFOREST = "Deforest"
    MINING = "Mining"
    OIL_EXTRACTION = "OilExtraction"
    PRODUCT_MEDICINE = "ProductMedicine"
    COOL = "Cool"
    TRANSPORT = "Transport"
    MONSTER_FARM = "MonsterFarm"
