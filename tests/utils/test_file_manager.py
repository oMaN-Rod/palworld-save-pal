import os
from pathlib import Path
from uuid import uuid4

import pytest

from palworld_save_pal.utils.file_manager import FileManager, FileValidationResult


class TestFileValidationResult:
    def test_invalid_result(self):
        r = FileValidationResult(valid=False, error="Something wrong")
        assert r.valid is False
        assert r.error == "Something wrong"
        assert r.level_sav is None

    def test_valid_result(self):
        r = FileValidationResult(
            valid=True,
            level_sav="/path/Level.sav",
            level_meta="/path/LevelMeta.sav",
            players_dir="/path/Players",
        )
        assert r.valid is True
        assert r.level_sav == "/path/Level.sav"


class TestValidateSteamSaveDirectory:
    def test_valid_directory(self, tmp_path):
        save_dir = tmp_path / "save"
        save_dir.mkdir()
        (save_dir / "Level.sav").write_bytes(b"\x00")
        (save_dir / "LevelMeta.sav").write_bytes(b"\x00")
        players_dir = save_dir / "Players"
        players_dir.mkdir()
        player_uid = str(uuid4())
        (players_dir / f"{player_uid}.sav").write_bytes(b"\x00")

        result = FileManager.validate_steam_save_directory(
            str(save_dir / "Level.sav")
        )
        assert result.valid is True
        assert result.level_sav is not None
        assert result.players_dir is not None

    def test_missing_level_sav(self, tmp_path):
        save_dir = tmp_path / "save"
        save_dir.mkdir()
        result = FileManager.validate_steam_save_directory(
            str(save_dir / "Level.sav")
        )
        assert result.valid is False
        assert "Level.sav" in result.error

    def test_missing_players_dir(self, tmp_path):
        save_dir = tmp_path / "save"
        save_dir.mkdir()
        (save_dir / "Level.sav").write_bytes(b"\x00")
        result = FileManager.validate_steam_save_directory(
            str(save_dir / "Level.sav")
        )
        assert result.valid is False
        assert "Players" in result.error

    def test_no_player_saves(self, tmp_path):
        save_dir = tmp_path / "save"
        save_dir.mkdir()
        (save_dir / "Level.sav").write_bytes(b"\x00")
        (save_dir / "Players").mkdir()
        result = FileManager.validate_steam_save_directory(
            str(save_dir / "Level.sav")
        )
        assert result.valid is False
        assert "player save" in result.error.lower()

    def test_missing_level_meta_still_valid(self, tmp_path):
        save_dir = tmp_path / "save"
        save_dir.mkdir()
        (save_dir / "Level.sav").write_bytes(b"\x00")
        players_dir = save_dir / "Players"
        players_dir.mkdir()
        (players_dir / f"{uuid4()}.sav").write_bytes(b"\x00")
        result = FileManager.validate_steam_save_directory(
            str(save_dir / "Level.sav")
        )
        assert result.valid is True
        assert result.level_meta is None

    def test_global_pal_storage_detected(self, tmp_path):
        parent = tmp_path / "parent"
        save_dir = parent / "save"
        save_dir.mkdir(parents=True)
        (save_dir / "Level.sav").write_bytes(b"\x00")
        players_dir = save_dir / "Players"
        players_dir.mkdir()
        (players_dir / f"{uuid4()}.sav").write_bytes(b"\x00")
        (parent / "GlobalPalStorage.sav").write_bytes(b"\x00")

        result = FileManager.validate_steam_save_directory(
            str(save_dir / "Level.sav")
        )
        assert result.valid is True
        assert result.global_pal_storage_sav is not None


class TestGetPlayerSaves:
    def test_reads_player_files(self, tmp_path):
        uid = uuid4()
        (tmp_path / f"{uid}.sav").write_bytes(b"player_data")

        saves = FileManager.get_player_saves(str(tmp_path))
        assert uid in saves
        assert "sav" in saves[uid]
        assert saves[uid]["sav"] == b"player_data"

    def test_reads_dps_files(self, tmp_path):
        uid = uuid4()
        (tmp_path / f"{uid}_dps.sav").write_bytes(b"dps_data")

        saves = FileManager.get_player_saves(str(tmp_path))
        assert uid in saves
        assert "dps" in saves[uid]

    def test_skips_invalid_filenames(self, tmp_path):
        (tmp_path / "not_a_uuid.sav").write_bytes(b"data")
        saves = FileManager.get_player_saves(str(tmp_path))
        assert len(saves) == 0

    def test_empty_directory(self, tmp_path):
        saves = FileManager.get_player_saves(str(tmp_path))
        assert len(saves) == 0


class TestGetPlayerSavePaths:
    def test_returns_paths(self, tmp_path):
        uid = uuid4()
        sav_file = tmp_path / f"{uid}.sav"
        sav_file.write_bytes(b"data")

        paths = FileManager.get_player_save_paths(str(tmp_path))
        assert uid in paths
        assert "sav" in paths[uid]
        assert str(sav_file) == paths[uid]["sav"]
