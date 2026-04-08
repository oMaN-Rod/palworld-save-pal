from uuid import UUID

from palworld_save_pal.utils.uuid import (
    are_equal_uuids,
    is_empty_uuid,
    is_valid_uuid,
    parse_optional_uuid,
)


class TestIsValidUuid:
    def test_valid_uuid_string(self):
        assert is_valid_uuid("12345678-1234-1234-1234-123456789abc") is True

    def test_valid_uuid_object(self):
        assert is_valid_uuid(UUID("12345678-1234-1234-1234-123456789abc")) is True

    def test_invalid_string(self):
        assert is_valid_uuid("not-a-uuid") is False

    def test_none(self):
        assert is_valid_uuid(None) is False

    def test_empty_string(self):
        assert is_valid_uuid("") is False

    def test_integer(self):
        assert is_valid_uuid(12345) is False


class TestIsEmptyUuid:
    def test_empty_uuid_string(self):
        assert is_empty_uuid("00000000-0000-0000-0000-000000000000") is True

    def test_empty_uuid_object(self):
        assert is_empty_uuid(UUID("00000000-0000-0000-0000-000000000000")) is True

    def test_non_empty_uuid(self):
        assert is_empty_uuid("12345678-1234-1234-1234-123456789abc") is False

    def test_non_empty_uuid_object(self):
        assert is_empty_uuid(UUID("12345678-1234-1234-1234-123456789abc")) is False


class TestAreEqualUuids:
    def test_equal_strings(self):
        assert are_equal_uuids(
            "12345678-1234-1234-1234-123456789abc",
            "12345678-1234-1234-1234-123456789abc",
        )

    def test_equal_case_insensitive(self):
        assert are_equal_uuids(
            "12345678-1234-1234-1234-123456789ABC",
            "12345678-1234-1234-1234-123456789abc",
        )

    def test_equal_uuid_objects(self):
        a = UUID("12345678-1234-1234-1234-123456789abc")
        b = UUID("12345678-1234-1234-1234-123456789abc")
        assert are_equal_uuids(a, b)

    def test_mixed_types(self):
        a = UUID("12345678-1234-1234-1234-123456789abc")
        b = "12345678-1234-1234-1234-123456789abc"
        assert are_equal_uuids(a, b)

    def test_not_equal(self):
        assert not are_equal_uuids(
            "12345678-1234-1234-1234-123456789abc",
            "abcdef01-abcd-abcd-abcd-abcdef012345",
        )


class TestParseOptionalUuid:
    def test_valid_string(self):
        result = parse_optional_uuid("12345678-1234-1234-1234-123456789abc")
        assert result == UUID("12345678-1234-1234-1234-123456789abc")

    def test_uuid_object(self):
        uid = UUID("12345678-1234-1234-1234-123456789abc")
        result = parse_optional_uuid(uid)
        assert result == uid

    def test_none_returns_none(self):
        assert parse_optional_uuid(None) is None

    def test_empty_string_returns_none(self):
        assert parse_optional_uuid("") is None
