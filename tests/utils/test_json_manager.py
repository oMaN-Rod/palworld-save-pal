import json

import pytest

from palworld_save_pal.utils.json_manager import JsonManager, sanitize_string


class TestSanitizeString:
    def test_clean_string_unchanged(self):
        assert sanitize_string("hello world") == "hello world"

    def test_empty_string(self):
        assert sanitize_string("") == ""

    def test_unicode_string(self):
        assert sanitize_string("日本語テスト") == "日本語テスト"

    def test_surrogate_replaced(self):
        bad = "hello\ud800world"
        result = sanitize_string(bad)
        assert "\ud800" not in result
        assert "hello" in result
        assert "world" in result


class TestJsonManager:
    def test_read_empty_file(self, tmp_path):
        path = tmp_path / "test.json"
        path.write_text("{}", encoding="utf-8")
        jm = JsonManager(str(path))
        assert jm.read() == {}

    def test_write_and_read(self, tmp_path):
        path = tmp_path / "test.json"
        path.write_text("{}", encoding="utf-8")
        jm = JsonManager(str(path))
        jm.write({"key": "value", "num": 42})
        result = jm.read()
        assert result == {"key": "value", "num": 42}

    def test_append(self, tmp_path):
        path = tmp_path / "test.json"
        path.write_text('{"existing": 1}', encoding="utf-8")
        jm = JsonManager(str(path))
        jm.append("new_key", "new_value")
        result = jm.read()
        assert result["existing"] == 1
        assert result["new_key"] == "new_value"

    def test_delete(self, tmp_path):
        path = tmp_path / "test.json"
        path.write_text('{"a": 1, "b": 2}', encoding="utf-8")
        jm = JsonManager(str(path))
        jm.delete("a")
        result = jm.read()
        assert "a" not in result
        assert result["b"] == 2

    def test_delete_missing_key(self, tmp_path):
        path = tmp_path / "test.json"
        path.write_text('{"a": 1}', encoding="utf-8")
        jm = JsonManager(str(path))
        jm.delete("nonexistent")
        assert jm.read() == {"a": 1}

    def test_update_name(self, tmp_path):
        path = tmp_path / "test.json"
        path.write_text('{"item1": {"name": "old", "val": 1}}', encoding="utf-8")
        jm = JsonManager(str(path))
        jm.update_name("item1", "new_name")
        result = jm.read()
        assert result["item1"]["name"] == "new_name"
        assert result["item1"]["val"] == 1

    def test_update_name_missing_key(self, tmp_path):
        path = tmp_path / "test.json"
        path.write_text('{"a": 1}', encoding="utf-8")
        jm = JsonManager(str(path))
        jm.update_name("nonexistent", "name")
        assert jm.read() == {"a": 1}

    def test_ensure_file_exists_creates_file(self, tmp_path):
        path = tmp_path / "new_file.json"
        assert not path.exists()
        jm = JsonManager(str(path))
        assert path.exists()
        assert jm.read() == {}
