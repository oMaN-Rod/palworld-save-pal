from enum import Enum
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class PrefixedEnum(Enum):
    @classmethod
    def _prefix(cls):
        return getattr(cls, "_enum_prefix", f"{cls.__name__}::")

    def prefixed(self):
        return f"{self._enum_prefix.value}{self.value}"


class ArrayType(str, Enum):
    BYTE_PROPERTY = "ByteProperty"
    ENUM_PROPERTY = "EnumProperty"
    NAME_PROPERTY = "NameProperty"
    STRUCT_PROPERTY = "StructProperty"


class EntryState(str, Enum):
    NONE = "None"
    MODIFIED = "Modified"
    NEW = "New"
    DELETED = "Deleted"


class Element(str, Enum):
    """Element types in the game"""

    NEUTRAL = "Normal"
    DARK = "Dark"
    DRAGON = "Dragon"
    ICE = "Ice"
    FIRE = "Fire"
    GRASS = "Leaf"
    GROUND = "Earth"
    ELECTRIC = "Electricity"
    WATER = "Water"
    UNKNOWN = "Unknown"

    @classmethod
    def get_all_elements(cls) -> list[str]:
        """Get all element type values"""
        return [member.value for member in cls]

    @classmethod
    def from_value(cls, value: str) -> "Element":
        """Convert from game's enum format to our enum"""
        type_str = value.split("::")[-1]
        try:
            return next((t for t in cls if t.value == type_str), cls.UNKNOWN)
        except KeyError:
            return cls.UNKNOWN


class GroupType(str, PrefixedEnum):
    _enum_prefix = "EPalGroupType::"

    GUILD = "Guild"
    ORGANIZATION = "Organization"

    @staticmethod
    def from_value(value: str):
        try:
            value = value.replace(GroupType._enum_prefix.value, "")
            return GroupType(value)
        except Exception:
            logger.warning("%s is not a valid group type", value)


class PalGender(str, PrefixedEnum):
    _enum_prefix = "EPalGenderType::"

    MALE = "Male"
    FEMALE = "Female"

    @staticmethod
    def from_value(value: str):
        try:
            value = value.replace(PalGender._enum_prefix.value, "")
            return PalGender(value)
        except Exception:
            logger.warning("%s is not a valid gender, defaulting to female", value)
            return PalGender.FEMALE


class PalRank(int, Enum):
    RANK0 = 1
    RANK1 = 2
    RANK2 = 3
    RANK3 = 4
    RANK4 = 5

    def get_index(self):
        return self.value - 1

    @staticmethod
    def from_value(value: int):
        try:
            return PalRank(value)
        except Exception:
            logger.warning("%s is not a valid rank", value)


class WorkSuitability(str, PrefixedEnum):
    _enum_prefix = "EPalWorkSuitability::"

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

    @staticmethod
    def from_value(value: str):
        try:
            value = value.replace(WorkSuitability._enum_prefix.value, "")
            return WorkSuitability(value)
        except Exception:
            logger.warning("%s is not a valid work suitability", value)
