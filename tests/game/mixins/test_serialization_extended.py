"""Extended integration tests for SerializationMixin."""

import json
import os
import tempfile

import pytest

from palworld_save_tools.gvas import GvasFile
from palworld_save_tools.palsav import decompress_sav_to_gvas
from palworld_save_tools.paltypes import PALWORLD_TYPE_HINTS

from palworld_save_pal.game.gvas_codec import CUSTOM_PROPERTIES
from tests.game.conftest import GPS_FILE, PLAYER_O_UID, WORLD1_DIR, _noop


@pytest.fixture
def sm_with_player(event_loop, fresh_save_manager):
    event_loop.run_until_complete(
        fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
    )
    return fresh_save_manager


@pytest.fixture
def sm_with_gps(fresh_save_manager):
    with open(GPS_FILE, "rb") as f:
        fresh_save_manager.load_gps(f.read())
    return fresh_save_manager


@pytest.fixture
def sm_with_level_meta(fresh_save_manager):
    meta_path = WORLD1_DIR / "LevelMeta.sav"
    if not meta_path.exists():
        pytest.skip("LevelMeta.sav not available")
    with open(meta_path, "rb") as f:
        fresh_save_manager.load_level_meta(f.read())
    return fresh_save_manager


# ---------------------------------------------------------------------------
# get_json
# ---------------------------------------------------------------------------
class TestGetJson:
    def test_get_json_returns_string(self, fresh_save_manager):
        result = fresh_save_manager.get_json()
        assert isinstance(result, str)
        assert len(result) > 0

    def test_get_json_is_valid_json(self, fresh_save_manager):
        result = fresh_save_manager.get_json(allow_nan=False)
        parsed = json.loads(result)
        assert isinstance(parsed, dict)

    def test_get_json_minified_is_shorter(self, fresh_save_manager):
        full = fresh_save_manager.get_json(minify=False)
        mini = fresh_save_manager.get_json(minify=True)
        assert len(mini) < len(full)

    def test_get_json_minified_has_no_indentation(self, fresh_save_manager):
        mini = fresh_save_manager.get_json(minify=True)
        # Minified JSON has no newlines between keys
        assert "\n" not in mini

    def test_get_json_contains_world_save_data(self, fresh_save_manager):
        result = fresh_save_manager.get_json()
        assert "worldSaveData" in result


# ---------------------------------------------------------------------------
# get_dict
# ---------------------------------------------------------------------------
class TestGetDict:
    def test_get_dict_returns_dict(self, fresh_save_manager):
        result = fresh_save_manager.get_dict()
        assert isinstance(result, dict)

    def test_get_dict_has_properties(self, fresh_save_manager):
        result = fresh_save_manager.get_dict()
        assert "properties" in result

    def test_get_dict_has_header(self, fresh_save_manager):
        result = fresh_save_manager.get_dict()
        assert "header" in result


# ---------------------------------------------------------------------------
# load_json
# ---------------------------------------------------------------------------
class TestLoadJson:
    def test_load_json_roundtrip(self, fresh_save_manager):
        json_str = fresh_save_manager.get_json(allow_nan=True)
        json_bytes = json_str.encode("utf-8")
        result = fresh_save_manager.load_json(json_bytes)
        assert result is not None
        # Should still be able to get dict after reloading
        d = fresh_save_manager.get_dict()
        assert isinstance(d, dict)


# ---------------------------------------------------------------------------
# load_level_meta
# ---------------------------------------------------------------------------
class TestLoadLevelMeta:
    def test_load_level_meta(self, fresh_save_manager):
        meta_path = WORLD1_DIR / "LevelMeta.sav"
        if not meta_path.exists():
            pytest.skip("LevelMeta.sav not available")
        with open(meta_path, "rb") as f:
            data = f.read()
        result = fresh_save_manager.load_level_meta(data)
        assert result is not None


# ---------------------------------------------------------------------------
# load_level_sav
# ---------------------------------------------------------------------------
class TestLoadLevelSav:
    def test_load_level_sav(self, fresh_save_manager):
        with open(WORLD1_DIR / "Level.sav", "rb") as f:
            data = f.read()
        result = fresh_save_manager.load_level_sav(data)
        assert result is not None


# ---------------------------------------------------------------------------
# convert_sav_file_to_json
# ---------------------------------------------------------------------------
class TestConvertSavToJson:
    def test_convert_sav_to_json(self, fresh_save_manager):
        with open(WORLD1_DIR / "Level.sav", "rb") as f:
            data = f.read()
        result = fresh_save_manager.convert_sav_file_to_json(data)
        assert isinstance(result, str)
        parsed = json.loads(result)
        assert isinstance(parsed, dict)

    def test_convert_sav_to_json_minified(self, fresh_save_manager):
        with open(WORLD1_DIR / "Level.sav", "rb") as f:
            data = f.read()
        result = fresh_save_manager.convert_sav_file_to_json(data, minify=True)
        assert "\n" not in result

    def test_convert_sav_to_json_not_minified(self, fresh_save_manager):
        with open(WORLD1_DIR / "Level.sav", "rb") as f:
            data = f.read()
        result = fresh_save_manager.convert_sav_file_to_json(data, minify=False)
        assert "\n" in result


# ---------------------------------------------------------------------------
# convert_json_to_sav_file
# ---------------------------------------------------------------------------
class TestConvertJsonToSav:
    def test_convert_json_to_sav_roundtrip(self, fresh_save_manager):
        with open(WORLD1_DIR / "Level.sav", "rb") as f:
            original_sav = f.read()
        json_str = fresh_save_manager.convert_sav_file_to_json(original_sav)
        sav_bytes = fresh_save_manager.convert_json_to_sav_file(
            json_str.encode("utf-8")
        )
        assert isinstance(sav_bytes, bytes)
        assert len(sav_bytes) > 0

        # Verify the roundtripped SAV can be parsed
        raw_gvas, _ = decompress_sav_to_gvas(sav_bytes)
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        assert gvas_file is not None


# ---------------------------------------------------------------------------
# level_meta_sav
# ---------------------------------------------------------------------------
class TestLevelMetaSav:
    def test_level_meta_sav_returns_bytes(self, sm_with_level_meta):
        result = sm_with_level_meta.level_meta_sav()
        assert isinstance(result, bytes)
        assert len(result) > 0

    def test_level_meta_sav_none_when_not_loaded(self):
        from palworld_save_pal.game.save_manager import SaveManager

        sm = SaveManager()
        result = sm.level_meta_sav()
        assert result is None

    def test_level_meta_sav_reparseable(self, sm_with_level_meta):
        sav_bytes = sm_with_level_meta.level_meta_sav()
        raw_gvas, _ = decompress_sav_to_gvas(sav_bytes)
        assert raw_gvas is not None
        assert len(raw_gvas) > 0


# ---------------------------------------------------------------------------
# gps_sav
# ---------------------------------------------------------------------------
class TestGpsSav:
    def test_gps_sav_returns_bytes(self, sm_with_gps):
        result = sm_with_gps.gps_sav()
        assert isinstance(result, bytes)
        assert len(result) > 0

    def test_gps_sav_none_when_not_loaded(self, fresh_save_manager):
        result = fresh_save_manager.gps_sav()
        assert result is None

    def test_gps_sav_reparseable(self, sm_with_gps):
        sav_bytes = sm_with_gps.gps_sav()
        raw_gvas, _ = decompress_sav_to_gvas(sav_bytes)
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        assert gvas_file is not None


# ---------------------------------------------------------------------------
# to_json_file
# ---------------------------------------------------------------------------
class TestToJsonFile:
    def test_to_json_file_creates_file(self, fresh_save_manager):
        with tempfile.TemporaryDirectory() as tmpdir:
            output = os.path.join(tmpdir, "output.json")
            fresh_save_manager.to_json_file(output, minify=False, allow_nan=True)
            assert os.path.exists(output)
            with open(output, "r", encoding="utf-8") as f:
                data = json.load(f)
            assert isinstance(data, dict)

    def test_to_json_file_minified(self, fresh_save_manager):
        with tempfile.TemporaryDirectory() as tmpdir:
            output = os.path.join(tmpdir, "output_mini.json")
            fresh_save_manager.to_json_file(output, minify=True, allow_nan=True)
            with open(output, "r", encoding="utf-8") as f:
                content = f.read()
            assert "\n" not in content


# ---------------------------------------------------------------------------
# to_level_sav_file
# ---------------------------------------------------------------------------
class TestToLevelSavFile:
    def test_to_level_sav_file_creates_file(self, fresh_save_manager):
        with tempfile.TemporaryDirectory() as tmpdir:
            output = os.path.join(tmpdir, "Level.sav")
            fresh_save_manager.to_level_sav_file(output)
            assert os.path.exists(output)
            with open(output, "rb") as f:
                data = f.read()
            assert len(data) > 0

    def test_to_level_sav_file_reparseable(self, fresh_save_manager):
        with tempfile.TemporaryDirectory() as tmpdir:
            output = os.path.join(tmpdir, "Level.sav")
            fresh_save_manager.to_level_sav_file(output)
            with open(output, "rb") as f:
                sav_bytes = f.read()
            raw_gvas, _ = decompress_sav_to_gvas(sav_bytes)
            gvas_file = GvasFile.read(
                raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
            )
            assert gvas_file is not None


# ---------------------------------------------------------------------------
# to_level_meta_sav_file
# ---------------------------------------------------------------------------
class TestToLevelMetaSavFile:
    def test_to_level_meta_sav_file_creates_file(self, sm_with_level_meta):
        with tempfile.TemporaryDirectory() as tmpdir:
            output = os.path.join(tmpdir, "LevelMeta.sav")
            sm_with_level_meta.to_level_meta_sav_file(output)
            assert os.path.exists(output)
            with open(output, "rb") as f:
                data = f.read()
            assert len(data) > 0

    def test_to_level_meta_sav_file_no_meta_raises(self):
        from palworld_save_pal.game.save_manager import SaveManager

        sm = SaveManager()
        with tempfile.TemporaryDirectory() as tmpdir:
            output = os.path.join(tmpdir, "LevelMeta.sav")
            with pytest.raises(ValueError, match="No LevelMeta"):
                sm.to_level_meta_sav_file(output)


# ---------------------------------------------------------------------------
# to_player_sav_files
# ---------------------------------------------------------------------------
class TestToPlayerSavFiles:
    def test_to_player_sav_files_creates_files(self, sm_with_player):
        with tempfile.TemporaryDirectory() as tmpdir:
            sm_with_player.to_player_sav_files(tmpdir)
            sav_files = [f for f in os.listdir(tmpdir) if f.endswith(".sav")]
            assert len(sav_files) >= 1

    def test_to_player_sav_files_reparseable(self, sm_with_player):
        with tempfile.TemporaryDirectory() as tmpdir:
            sm_with_player.to_player_sav_files(tmpdir)
            sav_files = [f for f in os.listdir(tmpdir) if f.endswith(".sav")]
            for sav_file in sav_files:
                with open(os.path.join(tmpdir, sav_file), "rb") as f:
                    sav_bytes = f.read()
                raw_gvas, _ = decompress_sav_to_gvas(sav_bytes)
                gvas_file = GvasFile.read(
                    raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
                )
                assert gvas_file is not None
