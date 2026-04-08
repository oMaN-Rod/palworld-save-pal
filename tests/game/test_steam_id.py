from uuid import UUID

import pytest

from palworld_save_pal.game.steam_id import (
    is_palworld_uid,
    parse_palworld_uid,
    parse_steam_input,
    player_uid_to_nosteam,
    steam_id_to_player_uid,
)


class TestParseSteamInput:
    def test_plain_numeric(self):
        assert parse_steam_input("76561198012345678") == 76561198012345678

    def test_with_whitespace(self):
        assert parse_steam_input("  76561198012345678  ") == 76561198012345678

    def test_profile_url(self):
        result = parse_steam_input(
            "https://steamcommunity.com/profiles/76561198012345678/"
        )
        assert result == 76561198012345678

    def test_profile_url_no_trailing_slash(self):
        result = parse_steam_input(
            "https://steamcommunity.com/profiles/76561198012345678"
        )
        assert result == 76561198012345678

    def test_steam_prefix(self):
        assert parse_steam_input("steam_76561198012345678") == 76561198012345678

    def test_vanity_url_raises(self):
        with pytest.raises(ValueError, match="Vanity URLs"):
            parse_steam_input("https://steamcommunity.com/id/username")


class TestIsPalworldUid:
    def test_dashed_uuid(self):
        assert is_palworld_uid("12345678-1234-1234-1234-123456789abc") is True

    def test_hex_uuid(self):
        assert is_palworld_uid("12345678123412341234123456789abc") is True

    def test_invalid(self):
        assert is_palworld_uid("not-a-uuid") is False

    def test_empty(self):
        assert is_palworld_uid("") is False

    def test_with_whitespace(self):
        assert is_palworld_uid("  12345678123412341234123456789abc  ") is True


class TestParsePalworldUid:
    def test_dashed(self):
        result = parse_palworld_uid("12345678-1234-1234-1234-123456789abc")
        assert result == UUID("12345678-1234-1234-1234-123456789abc")

    def test_hex_format(self):
        result = parse_palworld_uid("12345678123412341234123456789abc")
        assert result == UUID("12345678-1234-1234-1234-123456789abc")


class TestSteamIdToPlayerUid:
    def test_deterministic(self):
        uid1 = steam_id_to_player_uid(76561198012345678)
        uid2 = steam_id_to_player_uid(76561198012345678)
        assert uid1 == uid2

    def test_different_ids_different_uids(self):
        uid1 = steam_id_to_player_uid(76561198012345678)
        uid2 = steam_id_to_player_uid(76561198099999999)
        assert uid1 != uid2

    def test_result_is_uuid(self):
        result = steam_id_to_player_uid(76561198012345678)
        assert isinstance(result, UUID)

    def test_trailing_bytes_are_zero(self):
        result = steam_id_to_player_uid(76561198012345678)
        assert result.bytes[4:] == b"\x00" * 12


class TestPlayerUidToNosteam:
    def test_returns_formatted_uuid_string(self):
        uid = UUID("12345678-0000-0000-0000-000000000000")
        result = player_uid_to_nosteam(uid)
        assert result.endswith("-0000-0000-0000-000000000000")
        assert len(result) == 36

    def test_deterministic(self):
        uid = UUID("AABBCCDD-0000-0000-0000-000000000000")
        r1 = player_uid_to_nosteam(uid)
        r2 = player_uid_to_nosteam(uid)
        assert r1 == r2
