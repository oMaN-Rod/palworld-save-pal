from enum import Enum


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
