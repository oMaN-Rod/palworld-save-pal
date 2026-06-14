import pytest

from palworld_save_pal.utils.indexed_collection import IndexedCollection


def _key_extractor(item):
    return item["id"]


def _make_collection(items=None):
    items = items or []
    return IndexedCollection(data=items, key_extractor=_key_extractor)


class TestIndexedCollectionBasics:
    def test_empty_collection(self):
        c = _make_collection()
        assert len(c) == 0
        assert not c
        assert list(c) == []

    def test_bool_truthy(self):
        c = _make_collection([{"id": 1, "name": "a"}])
        assert c

    def test_len(self):
        c = _make_collection([{"id": 1}, {"id": 2}, {"id": 3}])
        assert len(c) == 3

    def test_iter(self):
        items = [{"id": 1}, {"id": 2}]
        c = _make_collection(items)
        assert list(c) == items

    def test_data_property(self):
        items = [{"id": 1}]
        c = _make_collection(items)
        assert c.data is items


class TestIndexedCollectionLookup:
    def test_get_existing(self):
        c = _make_collection([{"id": 1, "val": "a"}, {"id": 2, "val": "b"}])
        assert c.get(1) == {"id": 1, "val": "a"}

    def test_get_missing(self):
        c = _make_collection([{"id": 1}])
        assert c.get(99) is None

    def test_contains(self):
        c = _make_collection([{"id": 1}, {"id": 2}])
        assert 1 in c
        assert 99 not in c


class TestIndexedCollectionLazyIndex:
    def test_index_not_built_on_init(self):
        c = _make_collection([{"id": 1}])
        assert c._index is None

    def test_index_built_on_first_access(self):
        c = _make_collection([{"id": 1}])
        _ = c.index
        assert c._index is not None

    def test_invalidate(self):
        c = _make_collection([{"id": 1}])
        _ = c.get(1)
        assert c._index is not None
        c.invalidate()
        assert c._index is None


class TestIndexedCollectionMutations:
    def test_add(self):
        c = _make_collection([{"id": 1}])
        key = c.add({"id": 2, "val": "new"})
        assert key == 2
        assert len(c) == 2
        assert c.get(2) == {"id": 2, "val": "new"}
        assert {"id": 2, "val": "new"} in c.data

    def test_add_before_index_built(self):
        c = _make_collection()
        c.add({"id": 1})
        assert c._index is None
        assert len(c) == 1

    def test_remove_by_key(self):
        c = _make_collection([{"id": 1}, {"id": 2}])
        removed = c.remove_by_key(1)
        assert removed is True
        assert len(c) == 1
        assert c.get(1) is None
        assert c.get(2) is not None

    def test_remove_by_key_missing(self):
        c = _make_collection([{"id": 1}])
        removed = c.remove_by_key(99)
        assert removed is False
        assert len(c) == 1

    def test_remove_item(self):
        item = {"id": 1}
        c = _make_collection([item, {"id": 2}])
        removed = c.remove(item)
        assert removed is True
        assert len(c) == 1

    def test_remove_item_not_present(self):
        c = _make_collection([{"id": 1}])
        removed = c.remove({"id": 99})
        assert removed is False


class TestIndexedCollectionEdgeCases:
    def test_key_extractor_error_skipped(self):
        items = [{"id": 1}, {"no_id": True}, {"id": 3}]
        c = _make_collection(items)
        assert c.get(1) is not None
        assert c.get(3) is not None
        assert len(c) == 3

    def test_none_key_skipped(self):
        c = IndexedCollection(
            data=[{"id": None}, {"id": 1}],
            key_extractor=_key_extractor,
        )
        assert c.get(1) is not None
        assert len(c.index) == 1

    def test_add_with_bad_key_extractor(self):
        c = _make_collection()
        key = c.add({"no_id_field": True})
        assert key is None
        assert len(c) == 1
