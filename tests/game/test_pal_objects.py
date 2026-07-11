from uuid import UUID

import pytest

from palworld_save_pal.game.enum import ArrayType, PalGender, WorkSuitability
from palworld_save_pal.game.pal_objects import PalObjects, toUUID

EMPTY_UUID = UUID("00000000-0000-0000-0000-000000000000")
TEST_UUID = UUID("12345678-1234-1234-1234-123456789abc")


class TestToUUID:
    def test_from_uuid(self):
        assert toUUID(TEST_UUID) == TEST_UUID

    def test_from_string(self):
        result = toUUID("12345678-1234-1234-1234-123456789abc")
        assert result == TEST_UUID

    def test_none_returns_none(self):
        assert toUUID(None) is None

    def test_int_returns_none(self):
        assert toUUID(12345) is None


# ---------------------------------------------------------------------------
# Property constructors
# ---------------------------------------------------------------------------


class TestPropertyConstructors:
    def test_str_property(self):
        p = PalObjects.StrProperty("hello")
        assert p["type"] == "StrProperty"
        assert p["value"] == "hello"
        assert p["id"] is None

    def test_name_property(self):
        p = PalObjects.NameProperty("test_name")
        assert p["type"] == "NameProperty"
        assert p["value"] == "test_name"

    def test_bool_property(self):
        p = PalObjects.BoolProperty(True)
        assert p["type"] == "BoolProperty"
        assert p["value"] is True

    def test_int_property(self):
        p = PalObjects.IntProperty(42)
        assert p["type"] == "IntProperty"
        assert p["value"] == 42

    def test_int64_property(self):
        p = PalObjects.Int64Property(9999999999)
        assert p["type"] == "Int64Property"
        assert p["value"] == 9999999999

    def test_float_property(self):
        p = PalObjects.FloatProperty(3.14)
        assert p["type"] == "FloatProperty"
        assert p["value"] == pytest.approx(3.14)

    def test_byte_property(self):
        p = PalObjects.ByteProperty(50)
        assert p["type"] == "ByteProperty"
        assert p["value"]["value"] == 50
        assert p["value"]["type"] == "None"

    def test_enum_property(self):
        p = PalObjects.EnumProperty("EPalGenderType", "Male")
        assert p["type"] == "EnumProperty"
        assert p["value"]["type"] == "EPalGenderType"
        assert p["value"]["value"] == "Male"


class TestGuid:
    def test_guid_from_string(self):
        g = PalObjects.Guid("12345678-1234-1234-1234-123456789abc")
        assert g["type"] == "StructProperty"
        assert g["struct_type"] == "Guid"
        assert g["value"] == TEST_UUID

    def test_guid_from_uuid(self):
        g = PalObjects.Guid(TEST_UUID)
        assert g["value"] == TEST_UUID

    def test_get_guid(self):
        g = PalObjects.Guid(TEST_UUID)
        assert PalObjects.get_guid(g) == TEST_UUID


# ---------------------------------------------------------------------------
# Nested get/set
# ---------------------------------------------------------------------------


class TestGetNested:
    def test_single_key(self):
        assert PalObjects.get_nested({"a": 1}, "a") == 1

    def test_multiple_keys(self):
        d = {"a": {"b": {"c": 42}}}
        assert PalObjects.get_nested(d, "a", "b", "c") == 42

    def test_missing_returns_default(self):
        assert PalObjects.get_nested({"a": 1}, "b", default="x") == "x"

    def test_missing_returns_none(self):
        assert PalObjects.get_nested({"a": 1}, "b") is None

    def test_none_intermediate(self):
        assert PalObjects.get_nested({"a": None}, "a", "b") is None


class TestSetNested:
    def test_set_existing(self):
        d = {"a": {"b": 1}}
        PalObjects.set_nested(d, "a", "b", value=99)
        assert d["a"]["b"] == 99

    def test_set_new_leaf(self):
        d = {"a": {"b": 1}}
        PalObjects.set_nested(d, "a", "new", value="hello")
        assert d["a"]["new"] == "hello"

    def test_missing_intermediate_raises(self):
        d = {"a": 1}
        with pytest.raises((KeyError, TypeError)):
            PalObjects.set_nested(d, "a", "b", value=2)


# ---------------------------------------------------------------------------
# Value get/set
# ---------------------------------------------------------------------------


class TestGetSetValue:
    def test_get_value(self):
        d = {"value": 42}
        assert PalObjects.get_value(d) == 42

    def test_get_value_default(self):
        assert PalObjects.get_value({}, default="x") == "x"

    def test_set_value(self):
        d = {"value": 1}
        PalObjects.set_value(d, 99)
        assert d["value"] == 99


# ---------------------------------------------------------------------------
# Byte/Enum property get/set
# ---------------------------------------------------------------------------


class TestByteEnumGetSet:
    def test_get_byte_property(self):
        p = PalObjects.ByteProperty(50)
        assert PalObjects.get_byte_property(p) == 50

    def test_set_byte_property(self):
        p = PalObjects.ByteProperty(50)
        PalObjects.set_byte_property(p, 100)
        assert PalObjects.get_byte_property(p) == 100

    def test_get_enum_property(self):
        p = PalObjects.EnumProperty("EPalGenderType", "Male")
        assert PalObjects.get_enum_property(p) == "Male"

    def test_set_enum_property(self):
        p = PalObjects.EnumProperty("EPalGenderType", "Male")
        PalObjects.set_enum_property(p, "Female")
        assert PalObjects.get_enum_property(p) == "Female"


# ---------------------------------------------------------------------------
# Array properties
# ---------------------------------------------------------------------------


class TestArrayProperties:
    def test_array_property_values(self):
        p = PalObjects.ArrayPropertyValues(
            ArrayType.ENUM_PROPERTY, ["a", "b"]
        )
        assert p["type"] == "ArrayProperty"
        assert p["array_type"] == "EnumProperty"
        assert PalObjects.get_array_property(p) == ["a", "b"]

    def test_array_property_empty(self):
        p = PalObjects.ArrayProperty(ArrayType.BYTE_PROPERTY)
        assert p["type"] == "ArrayProperty"
        assert p["value"] is None

    def test_array_property_with_custom_type(self):
        p = PalObjects.ArrayProperty(
            ArrayType.BYTE_PROPERTY, custom_type="custom"
        )
        assert p["custom_type"] == "custom"

    def test_get_array_property_empty(self):
        p = PalObjects.ArrayProperty(ArrayType.BYTE_PROPERTY)
        assert PalObjects.get_array_property(p) == []

    def test_append_array_item(self):
        p = PalObjects.ArrayPropertyValues(ArrayType.NAME_PROPERTY, ["a"])
        PalObjects.append_array_item(p, "b")
        assert PalObjects.get_array_property(p) == ["a", "b"]

    def test_pop_array_item(self):
        p = PalObjects.ArrayPropertyValues(ArrayType.NAME_PROPERTY, ["a", "b", "c"])
        popped = PalObjects.pop_array_item(p, 1)
        assert popped == "b"
        assert PalObjects.get_array_property(p) == ["a", "c"]

    def test_set_array_property(self):
        p = PalObjects.ArrayPropertyValues(ArrayType.NAME_PROPERTY, ["a"])
        PalObjects.set_array_property(p, ["x", "y"])
        assert PalObjects.get_array_property(p) == ["x", "y"]


# ---------------------------------------------------------------------------
# Compound properties
# ---------------------------------------------------------------------------


class TestFixedPoint64:
    def test_create(self):
        fp = PalObjects.FixedPoint64(545000)
        assert fp["struct_type"] == "FixedPoint64"
        assert fp["type"] == "StructProperty"

    def test_get(self):
        fp = PalObjects.FixedPoint64(545000)
        assert PalObjects.get_fixed_point64(fp) == 545000

    def test_set(self):
        fp = PalObjects.FixedPoint64(545000)
        PalObjects.set_fixed_point64(fp, 999000)
        assert PalObjects.get_fixed_point64(fp) == 999000


class TestPalContainerId:
    def test_create(self):
        c = PalObjects.PalContainerId(TEST_UUID)
        assert c["struct_type"] == "PalContainerId"
        assert PalObjects.get_pal_container_id(c) == TEST_UUID

    def test_set(self):
        c = PalObjects.PalContainerId(TEST_UUID)
        new_id = UUID("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        PalObjects.set_pal_container_id(c, new_id)
        assert PalObjects.get_pal_container_id(c) == new_id

    def test_set_invalid_raises(self):
        c = PalObjects.PalContainerId(TEST_UUID)
        with pytest.raises(ValueError):
            PalObjects.set_pal_container_id(c, 12345)


class TestPalCharacterSlotId:
    def test_create(self):
        s = PalObjects.PalCharacterSlotId(TEST_UUID, 3)
        assert s["struct_type"] == "PalCharacterSlotId"
        result = PalObjects.get_pal_character_slot_id(s)
        assert result == (TEST_UUID, 3)

    def test_set(self):
        s = PalObjects.PalCharacterSlotId(TEST_UUID, 0)
        new_id = UUID("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        PalObjects.set_pal_character_slot_id(s, new_id, 5)
        result = PalObjects.get_pal_character_slot_id(s)
        assert result == (new_id, 5)


class TestDateTime:
    def test_create(self):
        dt = PalObjects.DateTime(638486453957560000)
        assert dt["struct_type"] == "DateTime"
        assert dt["value"] == 638486453957560000


class TestVector:
    def test_create(self):
        v = PalObjects.Vector(1.0, 2.0, 3.0)
        assert v["struct_type"] == "Vector"
        assert v["value"]["x"] == 1.0
        assert v["value"]["y"] == 2.0
        assert v["value"]["z"] == 3.0


class TestMapProperty:
    def test_create(self):
        m = PalObjects.MapProperty("NameProperty", "IntProperty")
        assert m["type"] == "MapProperty"
        assert m["key_type"] == "NameProperty"
        assert m["value_type"] == "IntProperty"
        assert m["value"] == []

    def test_get_map_property(self):
        m = PalObjects.MapProperty("NameProperty", "IntProperty")
        assert PalObjects.get_map_property(m) == []


class TestWorkSuitabilityStruct:
    def test_create(self):
        ws = PalObjects.WorkSuitabilityStruct("EPalWorkSuitability::Mining", 5)
        assert ws["WorkSuitability"]["value"]["value"] == "EPalWorkSuitability::Mining"
        assert ws["Rank"]["value"] == 5


class TestStatusPointStruct:
    def test_create(self):
        sp = PalObjects.StatusPointStruct("最大HP", 100)
        assert sp["StatusName"]["value"] == "最大HP"
        assert sp["StatusPoint"]["value"] == 100


# ---------------------------------------------------------------------------
# PalSaveParameter
# ---------------------------------------------------------------------------


class TestPalSaveParameter:
    def test_structure(self, mock_pal_save_parameter):
        psp = mock_pal_save_parameter
        assert "key" in psp
        assert "value" in psp
        assert "CustomVersionData" in psp
        assert psp["key"]["InstanceId"]["value"] == UUID(
            "12345678-1234-1234-1234-123456789abc"
        )

    def test_character_id(self, mock_pal_save_parameter):
        psp = mock_pal_save_parameter
        save_param = psp["value"]["RawData"]["value"]["object"]["SaveParameter"]["value"]
        assert save_param["CharacterID"]["value"] == "Lambball"

    def test_gender(self, mock_pal_save_parameter):
        psp = mock_pal_save_parameter
        save_param = psp["value"]["RawData"]["value"]["object"]["SaveParameter"]["value"]
        assert save_param["Gender"]["value"]["value"] == "EPalGenderType::Male"

    def test_active_skills(self, mock_pal_save_parameter):
        psp = mock_pal_save_parameter
        save_param = psp["value"]["RawData"]["value"]["object"]["SaveParameter"]["value"]
        skills = PalObjects.get_array_property(save_param["EquipWaza"])
        assert skills == ["EPalWazaID::AirCanon"]

    def test_passive_skills(self, mock_pal_save_parameter):
        psp = mock_pal_save_parameter
        save_param = psp["value"]["RawData"]["value"]["object"]["SaveParameter"]["value"]
        passives = PalObjects.get_array_property(save_param["PassiveSkillList"])
        assert passives == ["Legend"]

    def test_default_work_suitability(self):
        psp = PalObjects.PalSaveParameter(
            character_id="Lambball",
            instance_id=UUID("12345678-1234-1234-1234-123456789abc"),
            owner_uid=UUID("abcdef01-abcd-abcd-abcd-abcdef012345"),
            container_id=UUID("11111111-2222-3333-4444-555555555555"),
            slot_idx=0,
            group_id=UUID("abcdef01-abcd-abcd-abcd-abcdef012345"),
        )
        save_param = psp["value"]["RawData"]["value"]["object"]["SaveParameter"]["value"]
        suitabilities = PalObjects.get_array_property(
            save_param["GotStatusPointList"]
        )
        assert len(suitabilities) == len(PalObjects.StatusNames)


class TestAsUuid:
    def test_from_uuid(self):
        assert PalObjects.as_uuid(TEST_UUID) == TEST_UUID

    def test_from_string(self):
        assert PalObjects.as_uuid("12345678-1234-1234-1234-123456789abc") == TEST_UUID

    def test_none(self):
        assert PalObjects.as_uuid(None) is None


class TestStatusNameMaps:
    def test_status_name_map_values_are_unique(self):
        values = list(PalObjects.StatusNameMap.values())
        assert len(values) == len(set(values)), "reverse map would be ambiguous"

    def test_ex_status_name_map_values_are_unique(self):
        values = list(PalObjects.ExStatusNameMap.values())
        assert len(values) == len(set(values)), "reverse map would be ambiguous"

    def test_move_speed_is_mapped(self):
        assert PalObjects.StatusNameMap["移動速度アップ"] == "move_speed"
