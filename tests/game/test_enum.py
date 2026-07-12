import pytest

from palworld_save_pal.game.enum import (
    ArrayType,
    Element,
    EntryState,
    GroupType,
    PalGender,
    PalRank,
    WazaID,
    WorkSuitability,
)


class TestArrayType:
    def test_values(self):
        assert ArrayType.BYTE_PROPERTY == "ByteProperty"
        assert ArrayType.ENUM_PROPERTY == "EnumProperty"
        assert ArrayType.NAME_PROPERTY == "NameProperty"
        assert ArrayType.STRUCT_PROPERTY == "StructProperty"


class TestEntryState:
    def test_values(self):
        assert EntryState.NONE == "None"
        assert EntryState.MODIFIED == "Modified"
        assert EntryState.NEW == "New"
        assert EntryState.DELETED == "Deleted"


class TestElement:
    def test_from_value_with_prefix(self):
        assert Element.from_value("EPalElementType::Fire") == Element.FIRE

    def test_from_value_without_prefix(self):
        assert Element.from_value("Fire") == Element.FIRE

    def test_from_value_unknown(self):
        assert Element.from_value("Nonexistent") == Element.UNKNOWN

    def test_get_all_elements(self):
        elements = Element.get_all_elements()
        assert "Fire" in elements
        assert "Normal" in elements
        assert len(elements) == len(Element)


class TestGroupType:
    def test_from_value_guild(self):
        assert GroupType.from_value("EPalGroupType::Guild") == GroupType.GUILD

    def test_from_value_plain(self):
        assert GroupType.from_value("Guild") == GroupType.GUILD

    def test_prefixed(self):
        assert GroupType.GUILD.prefixed() == "EPalGroupType::Guild"

    def test_from_value_invalid(self):
        assert GroupType.from_value("Invalid") is None


class TestPalGender:
    def test_from_value_male(self):
        assert PalGender.from_value("EPalGenderType::Male") == PalGender.MALE

    def test_from_value_plain(self):
        assert PalGender.from_value("Female") == PalGender.FEMALE

    def test_from_value_invalid_defaults_female(self):
        assert PalGender.from_value("Invalid") == PalGender.FEMALE

    def test_prefixed(self):
        assert PalGender.MALE.prefixed() == "EPalGenderType::Male"


class TestPalRank:
    def test_values(self):
        assert PalRank.RANK0.value == 1
        assert PalRank.RANK4.value == 5

    def test_get_index(self):
        assert PalRank.RANK0.get_index() == 0
        assert PalRank.RANK4.get_index() == 4

    def test_from_value(self):
        assert PalRank.from_value(1) == PalRank.RANK0
        assert PalRank.from_value(5) == PalRank.RANK4

    def test_from_value_invalid(self):
        assert PalRank.from_value(99) is None


class TestWorkSuitability:
    def test_from_value(self):
        assert WorkSuitability.from_value("EmitFlame") == WorkSuitability.EMIT_FLAME

    def test_from_value_with_prefix(self):
        result = WorkSuitability.from_value("EPalWorkSuitability::Mining")
        assert result == WorkSuitability.MINING

    def test_from_value_invalid(self):
        assert WorkSuitability.from_value("Invalid") is None

    def test_prefixed(self):
        assert (
            WorkSuitability.HANDCRAFT.prefixed() == "EPalWorkSuitability::Handcraft"
        )

    def test_all_members(self):
        assert len(WorkSuitability) == 14


class TestWazaID:
    def test_from_str_by_name(self):
        waza, skill_str = WazaID.from_str("EPalWazaID::AirCanon")
        assert waza == WazaID.AirCanon
        assert skill_str == "EPalWazaID::AirCanon"

    def test_from_str_by_number(self):
        waza, skill_str = WazaID.from_str("EPalWazaID::22")
        assert waza == WazaID.AirCanon
        assert skill_str == "EPalWazaID::AirCanon"

    def test_from_str_invalid_prefix(self):
        waza, skill_str = WazaID.from_str("SomethingElse::22")
        assert waza is None
        assert skill_str == "SomethingElse::22"

    def test_from_str_invalid_name(self):
        waza, skill_str = WazaID.from_str("EPalWazaID::NonexistentSkill")
        assert waza is None
        assert skill_str == "EPalWazaID::NonexistentSkill"

    def test_to_str(self):
        assert WazaID.AirCanon.to_str() == "EPalWazaID::AirCanon"
        assert WazaID.NONE.to_str() == "EPalWazaID::NONE"

    def test_int_value(self):
        assert WazaID.AirCanon == 22
        assert WazaID.NONE == 0
        assert WazaID.MAX == 391
