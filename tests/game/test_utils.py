import pytest

from palworld_save_pal.game.utils import format_character_key, get_pal_data


class TestFormatCharacterKey:
    def test_boss_prefix_stripped(self):
        result = format_character_key("Boss_Lambball")
        assert result == "lambball"

    def test_predator_prefix_stripped(self):
        result = format_character_key("Predator_SomeMonster")
        assert result == "somemonster"

    def test_avatar_suffix_stripped(self):
        result = format_character_key("Lambball_Avatar")
        assert result == "lambball"

    def test_normal_id_lowered(self):
        result = format_character_key("Lambball")
        assert result == "lambball"

    def test_already_lowercase(self):
        result = format_character_key("lambball")
        assert result == "lambball"


class TestGetPalData:
    def test_empty_key_returns_none(self):
        assert get_pal_data("") is None
        assert get_pal_data(None) is None

    def test_nonexistent_key_returns_none(self):
        assert get_pal_data("totally_fake_pal_that_doesnt_exist_xyz") is None

    def test_valid_key_returns_data(self):
        result = get_pal_data("anubis")
        assert result is not None
        assert isinstance(result, dict)
