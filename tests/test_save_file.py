import os
from uuid import UUID
import pytest
from pathlib import Path

from palworld_save_pal.state import get_app_state

# Assuming your test save files are stored in a 'test_saves' directory
SAVE_DIR = os.path.join(os.path.dirname(__file__), "fixtures")
SAVE_PATH = os.path.join(SAVE_DIR, "Level.sav")

app_state = get_app_state()
file = open(SAVE_PATH, "rb")
app_state.process_save_file(file.read())


def test_save_file_loading():
    assert app_state.save_file.name == "Level.sav"
    assert app_state.save_file.size > 0
    assert app_state.save_file._gvas_file is not None


def test_load_pals():
    assert app_state.save_file.pal_count() > 0
    for instance_id, pal_data in app_state.save_file.get_pals().items():
        assert isinstance(instance_id, UUID)
        assert pal_data.character_id is not None
        assert pal_data.owner_uid is not None
        assert pal_data.level is not None


def test_get_players_list():
    assert len(app_state.players.values()) > 0
    for player in app_state.players.values():
        assert isinstance(player.uid, UUID)
        assert isinstance(player.nickname, str)


def test_get_pal():
    assert len(app_state.players.values()) > 0
    # first pal of first player
    player = list(app_state.players.values())[0]
    pal_summary = list(player.pals.values())[0]
    pal = app_state.save_file.get_pal(pal_summary.instance_id)

    assert pal is not None
    assert pal.instance_id == pal_summary.instance_id
    assert isinstance(pal.instance_id, UUID)
    assert isinstance(pal.character_id, str)
    assert isinstance(pal.owner_uid, UUID)
    assert isinstance(pal.level, int)
    assert isinstance(pal.nickname, str)


def test_load_and_save_without_modifications():
    with open(SAVE_PATH, "rb") as f:
        original_content = f.read()

    # Compare the generated SAV with the original
    assert (
        app_state.save_file.sav() == original_content
    ), "Generated SAV file does not match the original"
