"""
IndexedCollection - A synchronized list+index data structure.

Maintains both a list (for persistence/serialization) and a dictionary index
(for O(1) lookups). All modifications through this class automatically keep
both structures in sync.
"""

from typing import (
    Callable,
    Dict,
    Generic,
    Iterator,
    List,
    Optional,
    TypeVar,
)

from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

K = TypeVar("K")  # Key type (typically UUID)
V = TypeVar("V")  # Value type (typically Dict[str, Any])


class IndexedCollection(Generic[K, V]):
    """
    A collection that maintains both a list (for persistence) and a dict index
    (for O(1) lookups). Modifications through this class automatically keep both in sync.

    The index is built lazily on first access and updated incrementally on mutations.

    Usage:
        # Create with a key extractor function
        items = IndexedCollection(
            data=save_data_list,
            key_extractor=lambda e: PalObjects.as_uuid(e["key"]["ID"])
        )

        # O(1) lookup
        item = items.get(some_uuid)

        # Add item (updates both list and index)
        items.add(new_item)

        # Remove by key (updates both list and index)
        items.remove_by_key(some_uuid)

        # Iterate over list
        for item in items:
            ...

        # Access underlying list directly (for serialization)
        raw_list = items.data
    """

    __slots__ = ("_data", "_key_extractor", "_index")

    def __init__(
        self,
        data: List[V],
        key_extractor: Callable[[V], Optional[K]],
    ):
        self._data = data
        self._key_extractor = key_extractor
        self._index: Optional[Dict[K, V]] = None

    @property
    def data(self) -> List[V]:
        return self._data

    @property
    def index(self) -> Dict[K, V]:
        if self._index is None:
            self._index = self._build_index()
        return self._index

    def _build_index(self) -> Dict[K, V]:
        result: Dict[K, V] = {}
        for item in self._data:
            try:
                key = self._key_extractor(item)
                if key is not None:
                    result[key] = item
            except (KeyError, TypeError):
                continue
        return result

    def get(self, key: K) -> Optional[V]:
        return self.index.get(key)

    def add(self, item: V) -> Optional[K]:
        self._data.append(item)
        try:
            key = self._key_extractor(item)
            if key is not None and self._index is not None:
                self._index[key] = item
            return key
        except (KeyError, TypeError):
            return None

    def remove_by_key(self, key: K) -> bool:
        item = self.index.get(key)
        if item is None:
            return False
        try:
            self._data.remove(item)
        except ValueError:
            logger.warning("Item with key %s found in index but not in data list", key)
            pass
        if self._index is not None and key in self._index:
            del self._index[key]
        return True

    def remove(self, item: V) -> bool:
        if item not in self._data:
            return False
        try:
            key = self._key_extractor(item)
        except (KeyError, TypeError):
            key = None
        self._data.remove(item)
        if key is not None and self._index is not None and key in self._index:
            del self._index[key]
        return True

    def invalidate(self) -> None:
        self._index = None

    def __contains__(self, key: K) -> bool:
        return key in self.index

    def __len__(self) -> int:
        return len(self._data)

    def __iter__(self) -> Iterator[V]:
        return iter(self._data)

    def __bool__(self) -> bool:
        return bool(self._data)
