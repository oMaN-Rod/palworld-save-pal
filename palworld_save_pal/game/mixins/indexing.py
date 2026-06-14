from typing import Any, Dict, List, Optional
from uuid import UUID

from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.indexed_collection import IndexedCollection
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class IndexingMixin:
    def invalidate_performance_caches(self) -> None:
        self._pal_owner_counts_cache = None
        self._player_guild_map_cache = None
        self._map_object_index = None
        if self._item_containers is not None:
            self._item_containers.invalidate()
        if self._dynamic_items is not None:
            self._dynamic_items.invalidate()
        if self._character_containers is not None:
            self._character_containers.invalidate()
        if self._character_save_parameters is not None:
            self._character_save_parameters.invalidate()
        logger.debug("Performance caches invalidated")

    def _get_map_object_index(self) -> Dict[UUID, List[Dict[str, Any]]]:
        if self._map_object_index is not None:
            return self._map_object_index
        self._map_object_index = self._build_map_object_index()
        return self._map_object_index

    def _build_map_object_index(self) -> Dict[UUID, List[Dict[str, Any]]]:
        index: Dict[UUID, List[Dict[str, Any]]] = {}
        if not self._map_object_save_data or "values" not in self._map_object_save_data:
            return index
        for map_object in self._map_object_save_data["values"]:
            try:
                base_camp_id = PalObjects.as_uuid(
                    map_object["Model"]["value"]["RawData"]["value"][
                        "base_camp_id_belong_to"
                    ]
                )
            except (KeyError, TypeError):
                continue
            if base_camp_id:
                if base_camp_id not in index:
                    index[base_camp_id] = []
                index[base_camp_id].append(map_object)
        return index

    def _get_item_containers(self) -> IndexedCollection[UUID, Dict[str, Any]]:
        if self._item_containers is not None:
            return self._item_containers
        self._item_containers = self._build_item_containers_collection()
        return self._item_containers

    def _build_item_containers_collection(
        self,
    ) -> IndexedCollection[UUID, Dict[str, Any]]:
        def key_extractor(entry: Dict[str, Any]) -> Optional[UUID]:
            try:
                return PalObjects.get_guid(entry["key"]["ID"])
            except (KeyError, TypeError):
                return None

        return IndexedCollection(
            data=self._item_container_save_data,
            key_extractor=key_extractor,
        )

    def _get_dynamic_items(self) -> IndexedCollection[UUID, Dict[str, Any]]:
        if self._dynamic_items is not None:
            return self._dynamic_items
        self._dynamic_items = self._build_dynamic_items_collection()
        return self._dynamic_items

    def _build_dynamic_items_collection(
        self,
    ) -> IndexedCollection[UUID, Dict[str, Any]]:
        def key_extractor(entry: Dict[str, Any]) -> Optional[UUID]:
            try:
                return PalObjects.as_uuid(
                    entry["RawData"]["value"]["id"]["local_id_in_created_world"]
                )
            except (KeyError, TypeError):
                return None

        return IndexedCollection(
            data=self._dynamic_item_save_data,
            key_extractor=key_extractor,
        )

    def _get_character_containers(self) -> IndexedCollection[UUID, Dict[str, Any]]:
        if self._character_containers is not None:
            return self._character_containers
        self._character_containers = self._build_character_containers_collection()
        return self._character_containers

    def _build_character_containers_collection(
        self,
    ) -> IndexedCollection[UUID, Dict[str, Any]]:
        def key_extractor(entry: Dict[str, Any]) -> Optional[UUID]:
            try:
                return PalObjects.get_guid(entry["key"]["ID"])
            except (KeyError, TypeError):
                return None

        return IndexedCollection(
            data=self._character_container_save_data,
            key_extractor=key_extractor,
        )

    def _get_character_save_parameters(self) -> IndexedCollection[UUID, Dict[str, Any]]:
        if self._character_save_parameters is not None:
            return self._character_save_parameters
        self._character_save_parameters = (
            self._build_character_save_parameters_collection()
        )
        return self._character_save_parameters

    def _build_character_save_parameters_collection(
        self,
    ) -> IndexedCollection[UUID, Dict[str, Any]]:
        def key_extractor(entry: Dict[str, Any]) -> Optional[UUID]:
            try:
                return PalObjects.get_guid(entry["key"]["InstanceId"])
            except (KeyError, TypeError):
                return None

        return IndexedCollection(
            data=self._character_save_parameter_map,
            key_extractor=key_extractor,
        )