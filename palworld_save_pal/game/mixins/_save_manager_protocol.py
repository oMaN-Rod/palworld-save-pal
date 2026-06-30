"""Static typing surface shared by the ``SaveManager`` mixins.

The mixins in this package (``PlayerOpsMixin``, ``PalOpsMixin``, ...) are never
used standalone; they are composed into :class:`SaveManager`, which owns all the
state (``_players``, ``_pals``, the various ``*_save_data`` lists, etc.) and the
cross-cutting helper methods. Because a type checker analyses each mixin in
isolation, every ``self._players`` access would otherwise be flagged as
``reportAttributeAccessIssue`` ("Cannot access attribute ... for class
``PlayerOpsMixin*``").

This ``Protocol`` declares that shared surface once. Each mixin inherits from it
**only under ``TYPE_CHECKING``** (at runtime the base is plain ``object``), so the
checker sees the full set of attributes/methods while runtime behaviour and
Pydantic's model machinery are untouched. ``SaveManager`` already implements
everything declared here, so no ``# type: ignore`` is needed at the call sites.

Keep this in sync with ``SaveManager`` (state) and the mixins (helper methods)
when adding cross-mixin members.
"""

from __future__ import annotations

from typing import (
    TYPE_CHECKING,
    Any,
    Dict,
    List,
    Optional,
    Protocol,
    Tuple,
)
from uuid import UUID

if TYPE_CHECKING:
    from palworld_save_tools.gvas import GvasFile

    from palworld_save_pal.dto.pal import PalDTO
    from palworld_save_pal.dto.summary import GuildSummary, PlayerSummary
    from palworld_save_pal.game.guild import Guild
    from palworld_save_pal.game.pal import Pal
    from palworld_save_pal.game.player import Player, PlayerGvasFiles
    from palworld_save_pal.utils.indexed_collection import IndexedCollection


class SaveManagerProtocol(Protocol):
    """Structural view of ``SaveManager`` for use as a ``TYPE_CHECKING`` base."""

    # --- public fields ---------------------------------------------------
    level_sav_path: str
    size: int
    world_name: str

    # --- core state (mirrors SaveManager PrivateAttrs) -------------------
    _players: Dict[UUID, "Player"]
    _pals: Dict[UUID, "Pal"]
    _guilds: Dict[UUID, "Guild"]
    _gps_pals: Optional[Dict[int, "Pal"]]

    _gvas_file: Optional["GvasFile"]
    _level_meta_gvas_file: Optional["GvasFile"]
    _player_gvas_files: Dict[UUID, "PlayerGvasFiles"]
    _gps_gvas_file: Optional["GvasFile"]

    _character_save_parameter_map: List[Dict[str, Any]]
    _character_save_parameters: Optional["IndexedCollection[UUID, Dict[str, Any]]"]
    _item_container_save_data: List[Dict[str, Any]]
    _item_containers: Optional["IndexedCollection[UUID, Dict[str, Any]]"]
    _dynamic_item_save_data: List[Dict[str, Any]]
    _dynamic_items: Optional["IndexedCollection[UUID, Dict[str, Any]]"]
    _character_container_save_data: List[Dict[str, Any]]
    _character_containers: Optional["IndexedCollection[UUID, Dict[str, Any]]"]
    _group_save_data_map: List[Dict[str, Any]]
    _base_camp_save_data_map: List[Dict[str, Any]]
    _map_object_save_data: List[Dict[str, Any]]
    _guild_extra_save_data_map: List[Dict[str, Any]]

    _player_summaries: Dict[UUID, Any]
    _guild_summaries: Dict[UUID, Any]
    _loaded_players: set
    _loaded_guilds: set
    _player_file_refs: Dict[UUID, Dict[str, Any]]
    _player_gvas_sav_cache: Dict[UUID, "GvasFile"]

    _pal_owner_counts_cache: Optional[Dict[UUID, int]]
    _player_guild_map_cache: Optional[Dict[UUID, UUID]]
    _map_object_index: Optional[Dict[UUID, List[Dict[str, Any]]]]

    # --- SaveManager helpers --------------------------------------------
    def get_character_container(self, container_id: UUID) -> Dict[str, Any]: ...
    def get_item_container(self, container_id: UUID) -> Dict[str, Any]: ...
    def _get_file_size(self, data: bytes) -> Any: ...
    def _get_player_pals(self, uid: UUID) -> Dict[UUID, "Pal"]: ...
    def _is_player(self, entry: Dict[str, Any]) -> bool: ...
    def _player_guild(self, player_id: UUID) -> Optional["Guild"]: ...
    def set_world_name(self, name: str) -> None: ...

    # --- IndexingMixin ---------------------------------------------------
    def invalidate_performance_caches(self) -> None: ...
    def _get_map_object_index(self) -> Dict[UUID, List[Dict[str, Any]]]: ...
    def _build_map_object_index(self) -> Dict[UUID, List[Dict[str, Any]]]: ...
    def _get_item_containers(self) -> "IndexedCollection[UUID, Dict[str, Any]]": ...
    def _build_item_containers_collection(
        self,
    ) -> "IndexedCollection[UUID, Dict[str, Any]]": ...
    def _get_dynamic_items(self) -> "IndexedCollection[UUID, Dict[str, Any]]": ...
    def _build_dynamic_items_collection(
        self,
    ) -> "IndexedCollection[UUID, Dict[str, Any]]": ...
    def _get_character_containers(
        self,
    ) -> "IndexedCollection[UUID, Dict[str, Any]]": ...
    def _build_character_containers_collection(
        self,
    ) -> "IndexedCollection[UUID, Dict[str, Any]]": ...
    def _get_character_save_parameters(
        self,
    ) -> "IndexedCollection[UUID, Dict[str, Any]]": ...
    def _build_character_save_parameters_collection(
        self,
    ) -> "IndexedCollection[UUID, Dict[str, Any]]": ...

    # --- LoadingMixin ----------------------------------------------------
    def _find_player_guild_id(self, player_id: UUID) -> Any: ...
    def _pal_belongs_to_player(
        self, entry: Dict[str, Any], player_id: UUID
    ) -> bool: ...
    async def load_player_on_demand(
        self, player_id: UUID, ws_callback: Any = None
    ) -> Optional["Player"]: ...
    def _load_player_pals_only(self, player_id: UUID) -> Dict[UUID, "Pal"]: ...
    def _load_guild_by_id(self, guild_id: UUID) -> Optional["Guild"]: ...
    def _load_bases_for_guild(self, guild_id: UUID) -> None: ...
    def _load_pals_for_container(self, container_id: UUID) -> Dict[UUID, "Pal"]: ...
    def _load_gps_pals(self) -> Any: ...

    # --- PalOpsMixin -----------------------------------------------------
    def _find_first_empty_gps_slot(self) -> Optional[int]: ...
    def add_gps_pal_from_dto(
        self, pal_dto: "PalDTO", storage_slot: Optional[int] = None
    ) -> Optional[Tuple[int, "Pal"]]: ...
    def delete_guild_pals(
        self, guild_id: UUID, base_id: UUID, pal_ids: List[UUID]
    ) -> None: ...
    def _delete_pal_by_id(self, pal_id: UUID) -> None: ...

    # --- PlayerOpsMixin --------------------------------------------------
    async def _delete_player_and_pals(
        self, player_id: UUID, ws_callback: Any
    ) -> Tuple[List[UUID], List[UUID]] | None: ...

    # --- GuildOpsMixin ---------------------------------------------------
    def _should_delete_map_object(
        self, map_object: dict, guild_id: UUID | None, player_ids: List[UUID]
    ) -> bool: ...
    def _delete_item_containers(
        self, target_id: UUID, container_ids_to_delete: List[UUID]
    ) -> None: ...
    def _delete_dynamic_items(self, item_container: UUID) -> None: ...
    def _delete_character_containers(
        self, container_ids_to_delete: List[UUID]
    ) -> None: ...

    # --- SummariesMixin --------------------------------------------------
    def _extract_player_summaries(self) -> Dict[UUID, "PlayerSummary"]: ...
    def _extract_guild_summaries(self) -> Dict[UUID, "GuildSummary"]: ...
    def _categorize_character_entries(
        self,
    ) -> Tuple[List[Tuple[UUID, Dict[str, Any]]], Dict[UUID, int]]: ...
    def _get_player_guild_map(self) -> Dict[UUID, UUID]: ...
    def _build_player_guild_index(self) -> Dict[UUID, UUID]: ...
    def _create_player_summary(
        self,
        uid: UUID,
        save_parameter: Dict[str, Any],
        player_guild_map: Dict[UUID, UUID],
        pal_owner_counts: Dict[UUID, int],
    ) -> "PlayerSummary": ...
    def _extract_players_sequential(
        self,
        players_data: List[Tuple[UUID, Dict[str, Any]]],
        player_guild_map: Dict[UUID, UUID],
        pal_owner_counts: Dict[UUID, int],
    ) -> Dict[UUID, "PlayerSummary"]: ...
    def _extract_players_parallel(
        self,
        players_data: List[Tuple[UUID, Dict[str, Any]]],
        player_guild_map: Dict[UUID, UUID],
        pal_owner_counts: Dict[UUID, int],
    ) -> Dict[UUID, "PlayerSummary"]: ...

    # --- PlayerSwapMixin -------------------------------------------------
    def rebuild_player_caches(self) -> None: ...
    async def _validate_swap_players(
        self, old_player_uid: UUID, new_player_uid: UUID
    ) -> Optional[Dict[str, Any]]: ...
    def _swap_player_gvas_uids(
        self,
        old_save_data: Dict[str, Any],
        new_save_data: Dict[str, Any],
        old_uid_str: str,
        new_uid_str: str,
    ) -> None: ...
    def _swap_character_save_parameters(
        self,
        old_uid_str: str,
        new_uid_str: str,
        old_instance_id: UUID,
        new_instance_id: UUID,
    ) -> None: ...
    def _swap_guild_member_uids(
        self,
        old_uid_str: str,
        new_uid_str: str,
        old_instance_id: UUID,
        new_instance_id: UUID,
    ) -> None: ...
    def _swap_guild_character_handles(
        self,
        raw_data: Dict[str, Any],
        old_uid_str: str,
        new_uid_str: str,
        old_instance_id: UUID,
        new_instance_id: UUID,
    ) -> None: ...
    def _swap_guild_admin_uid(
        self, raw_data: Dict[str, Any], old_uid_str: str, new_uid_str: str
    ) -> None: ...
    def _swap_guild_player_list(
        self, raw_data: Dict[str, Any], old_uid_str: str, new_uid_str: str
    ) -> None: ...
    def _swap_player_file_refs(
        self, old_player_uid: UUID, new_player_uid: UUID
    ) -> None: ...