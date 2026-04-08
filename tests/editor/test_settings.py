"""Tests for Settings editor class."""

import pytest

from palworld_save_pal.dto.settings import SettingsDTO
from palworld_save_pal.editor.settings import Settings


class _FakeSettingsModel:
    """Mimics SettingsModel returned by get_settings()."""

    def __init__(
        self,
        language="en",
        save_dir="/fake/path",
        clone_prefix="CP",
        new_pal_prefix="NP",
        debug_mode=False,
        cheat_mode=False,
    ):
        self.language = language
        self.save_dir = save_dir
        self.clone_prefix = clone_prefix
        self.new_pal_prefix = new_pal_prefix
        self.debug_mode = debug_mode
        self.cheat_mode = cheat_mode


@pytest.fixture
def patch_db(monkeypatch):
    """Monkeypatch DB calls so Settings never touches a real database."""
    fake = _FakeSettingsModel()
    saved_dtos = []

    monkeypatch.setattr(
        "palworld_save_pal.editor.settings.get_settings", lambda: fake
    )
    monkeypatch.setattr(
        "palworld_save_pal.editor.settings.update_settings",
        lambda dto: saved_dtos.append(dto),
    )
    monkeypatch.setattr(
        "palworld_save_pal.editor.settings.update_save_dir",
        lambda v: None,
    )

    return fake, saved_dtos


class TestSettingsInit:
    def test_defaults_from_db(self, patch_db):
        fake, _ = patch_db
        s = Settings()
        assert s.language == "en"
        assert s.save_dir == "/fake/path"
        assert s.clone_prefix == "CP"
        assert s.new_pal_prefix == "NP"
        assert s.debug_mode is False
        assert s.cheat_mode is False

    def test_custom_values_from_db(self, monkeypatch):
        fake = _FakeSettingsModel(
            language="ja",
            save_dir="/saves",
            clone_prefix="[C]",
            new_pal_prefix="[N]",
            debug_mode=True,
            cheat_mode=True,
        )
        monkeypatch.setattr(
            "palworld_save_pal.editor.settings.get_settings", lambda: fake
        )
        monkeypatch.setattr(
            "palworld_save_pal.editor.settings.update_settings", lambda dto: None
        )
        monkeypatch.setattr(
            "palworld_save_pal.editor.settings.update_save_dir", lambda v: None
        )

        s = Settings()
        assert s.language == "ja"
        assert s.save_dir == "/saves"
        assert s.clone_prefix == "[C]"
        assert s.new_pal_prefix == "[N]"
        assert s.debug_mode is True
        assert s.cheat_mode is True


class TestSettingsSetters:
    def test_set_language(self, patch_db):
        _, saved = patch_db
        s = Settings()
        s.language = "fr"
        assert s.language == "fr"
        assert len(saved) == 1
        assert saved[0].language == "fr"

    def test_set_clone_prefix(self, patch_db):
        _, saved = patch_db
        s = Settings()
        s.clone_prefix = "CLONE_"
        assert s.clone_prefix == "CLONE_"
        assert len(saved) == 1

    def test_set_new_pal_prefix(self, patch_db):
        _, saved = patch_db
        s = Settings()
        s.new_pal_prefix = "NEW_"
        assert s.new_pal_prefix == "NEW_"
        assert len(saved) == 1

    def test_set_debug_mode(self, patch_db):
        _, saved = patch_db
        s = Settings()
        s.debug_mode = True
        assert s.debug_mode is True
        assert saved[0].debug_mode is True

    def test_set_cheat_mode(self, patch_db):
        _, saved = patch_db
        s = Settings()
        s.cheat_mode = True
        assert s.cheat_mode is True
        assert saved[0].cheat_mode is True

    def test_set_save_dir(self, patch_db):
        s = Settings()
        s.save_dir = "/new/dir"
        assert s.save_dir == "/new/dir"


class TestSettingsUpdateFrom:
    def test_update_from_dto(self, patch_db):
        _, saved = patch_db
        s = Settings()

        dto = SettingsDTO(
            language="ko",
            clone_prefix="<<",
            new_pal_prefix=">>",
            debug_mode=True,
            cheat_mode=True,
        )
        s.update_from(dto)

        assert s.language == "ko"
        assert s.clone_prefix == "<<"
        assert s.new_pal_prefix == ">>"
        assert s.debug_mode is True
        assert s.cheat_mode is True
        # update_from calls update_settings directly
        assert len(saved) == 1
        assert saved[0] is dto

    def test_update_from_does_not_double_save(self, patch_db):
        """update_from sets _is_busy=True, so individual setters should NOT save."""
        _, saved = patch_db
        s = Settings()

        dto = SettingsDTO(
            language="de",
            clone_prefix="C",
            new_pal_prefix="N",
            debug_mode=False,
            cheat_mode=False,
        )
        s.update_from(dto)
        # Only one call to update_settings (from update_from itself)
        assert len(saved) == 1


class TestSettingsLoadFailure:
    def test_falls_back_to_defaults_on_error(self, monkeypatch):
        def raise_error():
            raise RuntimeError("DB unavailable")

        monkeypatch.setattr(
            "palworld_save_pal.editor.settings.get_settings", raise_error
        )
        monkeypatch.setattr(
            "palworld_save_pal.editor.settings.update_settings", lambda dto: None
        )
        monkeypatch.setattr(
            "palworld_save_pal.editor.settings.update_save_dir", lambda v: None
        )

        s = Settings()
        # Should fall back to class defaults
        assert s.language == "en"
        assert s.debug_mode is False
        assert s.cheat_mode is False
