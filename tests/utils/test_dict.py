import pytest

from palworld_save_pal.utils.dict import safe_get, safe_remove, safe_remove_multiple, safe_set


class TestSafeGet:
    def test_single_key(self):
        assert safe_get({"a": 1}, "a") == 1

    def test_nested_keys(self):
        d = {"a": {"b": {"c": 42}}}
        assert safe_get(d, "a", "b", "c") == 42

    def test_missing_key_returns_default(self):
        assert safe_get({"a": 1}, "b") is None

    def test_missing_key_custom_default(self):
        assert safe_get({"a": 1}, "b", default="fallback") == "fallback"

    def test_missing_nested_key(self):
        assert safe_get({"a": {"b": 1}}, "a", "c") is None

    def test_none_intermediate(self):
        assert safe_get({"a": None}, "a", "b") is None

    def test_empty_dict(self):
        assert safe_get({}, "a") is None


class TestSafeSet:
    def test_single_key(self):
        d = {"a": 1}
        safe_set(d, "a", value=2)
        assert d["a"] == 2

    def test_nested_keys(self):
        d = {"a": {"b": {"c": 1}}}
        safe_set(d, "a", "b", "c", value=99)
        assert d["a"]["b"]["c"] == 99

    def test_missing_intermediate_raises(self):
        d = {"a": 1}
        with pytest.raises((KeyError, TypeError)):
            safe_set(d, "a", "b", value=2)

    def test_missing_top_level_raises(self):
        d = {}
        with pytest.raises(KeyError):
            safe_set(d, "x", "y", value=1)

    def test_set_new_leaf_key(self):
        d = {"a": {"b": 1}}
        safe_set(d, "a", "new_key", value="hello")
        assert d["a"]["new_key"] == "hello"


class TestSafeRemove:
    def test_remove_single_key(self):
        d = {"a": 1, "b": 2}
        safe_remove(d, "a")
        assert "a" not in d
        assert d["b"] == 2

    def test_remove_nested_key(self):
        d = {"a": {"b": {"c": 1, "d": 2}}}
        safe_remove(d, "a", "b", "c")
        assert "c" not in d["a"]["b"]
        assert d["a"]["b"]["d"] == 2

    def test_remove_missing_key_no_error(self):
        d = {"a": 1}
        safe_remove(d, "nonexistent")
        assert d == {"a": 1}

    def test_remove_missing_nested_no_error(self):
        d = {"a": {"b": 1}}
        safe_remove(d, "a", "x", "y")
        assert d == {"a": {"b": 1}}


class TestSafeRemoveMultiple:
    def test_remove_multiple(self):
        d = {"a": 1, "b": 2, "c": 3}
        safe_remove_multiple(d, "a", "c")
        assert d == {"b": 2}

    def test_remove_with_missing_keys(self):
        d = {"a": 1, "b": 2}
        safe_remove_multiple(d, "a", "nonexistent")
        assert d == {"b": 2}
