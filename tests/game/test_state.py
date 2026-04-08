"""Tests for AppState."""

import asyncio
import threading
from uuid import UUID, uuid4

import pytest

from palworld_save_pal.game.gvas_codec import SaveType


@pytest.fixture
def patch_settings_db(monkeypatch):
    """Patch DB calls for Settings so AppState can be constructed without a DB."""

    class _FakeModel:
        language = "en"
        save_dir = "/fake"
        clone_prefix = "C"
        new_pal_prefix = "N"
        debug_mode = False
        cheat_mode = False

    monkeypatch.setattr(
        "palworld_save_pal.editor.settings.get_settings", lambda: _FakeModel()
    )
    monkeypatch.setattr(
        "palworld_save_pal.editor.settings.update_settings", lambda dto: None
    )
    monkeypatch.setattr(
        "palworld_save_pal.editor.settings.update_save_dir", lambda v: None
    )


@pytest.fixture
def app_state(patch_settings_db):
    from palworld_save_pal.state import AppState

    return AppState()


@pytest.fixture
def loop():
    l = asyncio.new_event_loop()
    yield l
    l.close()


class TestAppStateConstruction:
    def test_default_save_file_none(self, app_state):
        assert app_state.save_file is None

    def test_default_save_type(self, app_state):
        assert app_state.save_type == SaveType.STEAM

    def test_default_players_empty(self, app_state):
        assert app_state.players == {}

    def test_default_guilds_empty(self, app_state):
        assert app_state.guilds == {}

    def test_default_local_false(self, app_state):
        assert app_state.local is False

    def test_settings_initialized(self, app_state):
        assert app_state.settings is not None
        assert app_state.settings.language == "en"

    def test_terminate_flag_type(self, app_state):
        assert isinstance(app_state.terminate_flag, threading.Event)
        assert not app_state.terminate_flag.is_set()

    def test_webview_window_none(self, app_state):
        assert app_state.webview_window is None

    def test_player_summaries_empty(self, app_state):
        assert app_state.player_summaries == {}

    def test_guild_summaries_empty(self, app_state):
        assert app_state.guild_summaries == {}

    def test_gps_defaults(self, app_state):
        assert app_state.gps == {}
        assert app_state.gps_loaded is False
        assert app_state.gps_file_path is None

    def test_source_save_file_none(self, app_state):
        assert app_state.source_save_file is None

    def test_target_transfer_save_none(self, app_state):
        assert app_state.target_transfer_save is None


class TestAppStatePlayerLoaded:
    def test_not_loaded_initially(self, app_state):
        uid = uuid4()
        assert app_state.is_player_loaded(uid) is False

    def test_loaded_after_adding(self, app_state):
        uid = uuid4()
        # Simulate having loaded a player into the cache
        app_state.players[uid] = "fake_player"
        assert app_state.is_player_loaded(uid) is True


class TestAppStateGuildLoaded:
    def test_not_loaded_initially(self, app_state):
        uid = uuid4()
        assert app_state.is_guild_loaded(uid) is False

    def test_loaded_after_adding(self, app_state):
        uid = uuid4()
        app_state.guilds[uid] = "fake_guild"
        assert app_state.is_guild_loaded(uid) is True


class TestAppStateGamepassSave:
    def test_select_nonexistent_save(self, app_state):
        result = app_state.select_gamepass_save("nonexistent")
        assert result is None
        assert app_state.selected_gamepass_save is None

    def test_select_existing_save(self, app_state):
        fake_save = {"path": "/some/path"}
        app_state.gamepass_saves["save1"] = fake_save
        result = app_state.select_gamepass_save("save1")
        assert result is fake_save
        assert app_state.selected_gamepass_save is fake_save


class TestAppStateGpsAvailability:
    def test_no_gps_available(self, app_state):
        app_state.gps_file_path = None
        app_state.gps_loaded = False
        assert app_state.has_gps_available() is False

    def test_gps_file_path_set(self, app_state):
        app_state.gps_file_path = "/some/gps.sav"
        assert app_state.has_gps_available() is True

    def test_gps_already_loaded(self, app_state):
        app_state.gps_loaded = True
        assert app_state.has_gps_available() is True


class TestAppStateGetPlayerDetailsNoSave:
    def test_returns_none_without_save(self, app_state, loop):
        result = loop.run_until_complete(app_state.get_player_details(uuid4()))
        assert result is None


class TestAppStateGetGuildDetailsNoSave:
    def test_returns_none_without_save(self, app_state, loop):
        result = loop.run_until_complete(app_state.get_guild_details(uuid4()))
        assert result is None


class TestAppStateLoadGpsNoSave:
    def test_returns_none_without_save(self, app_state, loop):
        result = loop.run_until_complete(app_state.load_gps_on_demand())
        assert result is None

    def test_returns_none_without_gps_path(self, app_state, loop):
        app_state.save_file = "fake"  # non-None to pass first check
        app_state.gps_file_path = None
        result = loop.run_until_complete(app_state.load_gps_on_demand())
        assert result is None
