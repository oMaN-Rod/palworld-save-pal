from typing import TYPE_CHECKING, Dict, List, Optional, Tuple, Union
from uuid import UUID
import uuid

if TYPE_CHECKING:
    from palworld_save_pal.game.mixins._save_manager_protocol import (
        SaveManagerProtocol,
    )

    _Base = SaveManagerProtocol
else:
    _Base = object

from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.game.pal import Pal
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.game.enum import PalGender
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class PalOpsMixin(_Base):
    def get_pal(self, pal_id: UUID) -> Pal:
        return self._pals.get(pal_id)

    def get_pals(self):
        return self._pals

    def pal_count(self):
        return len(self._pals)

    def load_gps(self, global_pal_storage_sav: bytes):
        from palworld_save_tools.palsav import decompress_sav_to_gvas
        from palworld_save_tools.paltypes import PALWORLD_TYPE_HINTS
        from palworld_save_tools.gvas import GvasFile
        from palworld_save_pal.game.gvas_codec import CUSTOM_PROPERTIES
        from palworld_save_pal.utils.perf import gc_paused

        logger.info("Loading global pal storage")
        raw_gvas, _ = decompress_sav_to_gvas(global_pal_storage_sav)
        with gc_paused():
            gvas_file = GvasFile.read(
                raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
            )
        self._gps_gvas_file = gvas_file
        self._load_gps_pals()
        return self._gps_pals

    def get_gps(self) -> Optional[Dict[int, Pal]]:
        return self._gps_pals

    def add_player_pal(
        self,
        player_id: UUID,
        character_id: str,
        nickname: str,
        container_id: UUID,
        storage_slot: Union[int | None] = None,
    ) -> Optional[Pal]:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        new_pal = player.add_pal(character_id, nickname, container_id, storage_slot)
        if new_pal is None:
            return
        self._get_character_save_parameters().add(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        self.invalidate_performance_caches()
        return new_pal

    def add_player_pal_from_dto(
        self,
        player_id: UUID,
        pal_dto: PalDTO,
        container_id: UUID,
        storage_slot: Union[int | None] = None,
    ) -> Optional[Pal]:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        new_pal = player.add_pal_from_dto(pal_dto, container_id, storage_slot)
        if new_pal is None:
            return
        self._get_character_save_parameters().add(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        self.invalidate_performance_caches()
        return new_pal

    def add_player_dps_pal(
        self,
        player_id: UUID,
        character_id: str,
        nickname: str,
        storage_slot: Optional[int] = None,
    ) -> Optional[Tuple[int, Pal]]:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        slot_idx, new_pal = player.add_dps_pal(character_id, nickname, storage_slot)
        if new_pal is None:
            return
        return slot_idx, new_pal

    def add_player_dps_pal_from_dto(
        self,
        player_id: UUID,
        pal_dto: PalDTO,
        storage_slot: Optional[int] = None,
    ) -> Optional[Tuple[int, Pal]]:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        slot_idx, new_pal = player.add_dps_pal_from_dto(pal_dto, storage_slot)
        if new_pal is None:
            return
        return slot_idx, new_pal

    def _find_first_empty_gps_slot(self) -> Optional[int]:
        if not self._gps_gvas_file:
            raise ValueError("GPS Gvas file is not initialized.")

        save_parameter_array = PalObjects.get_array_property(
            self._gps_gvas_file.properties["SaveParameterArray"]
        )
        for index, entry in enumerate(save_parameter_array):
            save_parameter = PalObjects.get_value(entry["SaveParameter"])
            character_id = (
                PalObjects.get_value(save_parameter["CharacterID"])
                if save_parameter and "CharacterID" in save_parameter
                else None
            )
            if character_id is None or character_id == "None":
                logger.debug(
                    "Found empty GPS slot at index %s",
                    index,
                )
                return index
        return None

    def add_gps_pal(
        self,
        character_id: str,
        nickname: str,
        storage_slot: Optional[int] = None,
    ) -> Optional[Tuple[Pal, int]]:
        if not self._gps_gvas_file:
            raise ValueError("GPS Gvas file is not initialized.")

        slot_idx = (
            storage_slot
            if storage_slot is not None
            else self._find_first_empty_gps_slot()
        )
        if slot_idx is None:
            logger.error("No empty GPS slot found.")
            return None
        pal_data = PalObjects.get_array_property(
            self._gps_gvas_file.properties["SaveParameterArray"]
        )[slot_idx]

        pal = Pal(data=pal_data, dps=True)
        pal.reset()
        pal.owner_uid = PalObjects.EMPTY_UUID
        pal.instance_id = uuid.uuid4()
        pal.character_id = character_id
        pal.nickname = nickname
        pal.filtered_nickname = nickname
        pal.storage_id = PalObjects.EMPTY_UUID
        pal.storage_slot = 0
        pal.gender = PalGender.FEMALE
        pal.populate_status_point_lists()
        pal.hp = pal.max_hp
        if not self._gps_pals:
            self._gps_pals = {}
        self._gps_pals[slot_idx] = pal
        return pal, slot_idx

    def add_gps_pal_from_dto(
        self,
        pal_dto: PalDTO,
        storage_slot: Optional[int] = None,
    ) -> Optional[Tuple[int, Pal]]:
        if not self._gps_gvas_file:
            raise ValueError("GPS Gvas file is not initialized.")

        slot_idx = (
            storage_slot
            if storage_slot is not None
            else self._find_first_empty_gps_slot()
        )
        if slot_idx is None:
            logger.error("No empty GPS slot found.")
            return None
        pal_data = PalObjects.get_array_property(
            self._gps_gvas_file.properties["SaveParameterArray"]
        )[slot_idx]

        pal = Pal(data=pal_data, dps=True)
        new_pal_id = uuid.uuid4()
        pal_dto.owner_uid = PalObjects.EMPTY_UUID
        pal_dto.instance_id = new_pal_id
        pal_dto.storage_id = PalObjects.EMPTY_UUID
        pal_dto.storage_slot = 0
        pal.update_from(pal_dto)
        pal.populate_status_point_lists()
        if not self._gps_pals:
            self._gps_pals = {}
        self._gps_pals[slot_idx] = pal
        return slot_idx, pal

    def add_guild_pal(
        self,
        character_id: str,
        nickname: str,
        guild_id: UUID,
        base_id: UUID,
        storage_slot: Optional[int] = None,
    ):
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Guild {guild_id} not found in the save file.")
        new_pal = guild.add_base_pal(character_id, nickname, base_id, storage_slot)
        if new_pal is None:
            return
        self._get_character_save_parameters().add(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        self.invalidate_performance_caches()
        return new_pal

    def move_pal(self, player_id: UUID, pal_id: UUID, container_id: UUID) -> Pal | None:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        return player.move_pal(pal_id, container_id)

    def clone_pal(self, pal: PalDTO) -> Optional[Pal]:
        player = self._players.get(pal.owner_uid)
        if not player:
            raise ValueError(f"Player {pal.owner_uid} not found in the save file.")

        new_pal = player.clone_pal(pal)
        if new_pal is None:
            return
        self._get_character_save_parameters().add(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        self.invalidate_performance_caches()
        return new_pal

    def clone_dps_pal(self, pal: PalDTO) -> Optional[Tuple[int, Pal]]:
        player = self._players.get(pal.owner_uid)
        if not player:
            raise ValueError(f"Player {pal.owner_uid} not found in the save file.")

        res = player.clone_dps_pal(pal)
        if not res:
            return
        slot_idx, new_pal = res
        return slot_idx, new_pal

    def clone_gps_pal(self, pal: PalDTO) -> Optional[Tuple[int, Pal]]:
        return self.add_gps_pal_from_dto(pal)

    def clone_guild_pal(
        self, guild_id: UUID, base_id: UUID, pal: PalDTO
    ) -> Optional[Pal]:
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Base {base_id} not found in the guild {guild_id}.")
        new_pal = guild.clone_base_pal(base_id, pal)
        if new_pal is None:
            return
        self._get_character_save_parameters().add(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        self.invalidate_performance_caches()
        return new_pal

    def delete_player_pals(self, player_id: UUID, pal_ids: List[UUID]) -> None:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        for pal_id in pal_ids:
            player.delete_pal(pal_id)
            self._delete_pal_by_id(pal_id)

    def delete_player_dps_pals(self, player_id: UUID, pal_indexes: List[int]) -> None:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")
        player.delete_dps_pals(pal_indexes)

    def delete_guild_pals(
        self, guild_id: UUID, base_id: UUID, pal_ids: List[UUID]
    ) -> None:
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Base {base_id} not found in the guild {guild_id}.")

        for pal_id in pal_ids:
            guild.delete_base_pal(base_id, pal_id)
            self._delete_pal_by_id(pal_id)

    def heal_pals(self, pal_ids: List[UUID]) -> None:
        for pal_id in pal_ids:
            pal = self._pals.get(pal_id)
            if not pal:
                logger.error("Pal %s not found in the save file.", pal_id)
                continue
            pal.heal()

    def heal_all_player_pals(self, player_id: UUID) -> None:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")
        for pal in player.pals.values():
            pal.heal()

    def heal_all_base_pals(self, guild_id: UUID, base_id: UUID) -> None:
        base = self._guilds.get(guild_id).bases.get(base_id)
        if not base:
            raise ValueError(f"Base {base_id} not found in the guild {guild_id}.")
        for pal in base.pals.values():
            pal.heal()

    async def update_pals(self, modified_pals: Dict[UUID, PalDTO], ws_callback) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for pal_id, pal in modified_pals.items():
            pal_name = pal.nickname if pal.nickname else pal.character_id
            await ws_callback(f"Updating pal {pal_name}")
            existing_pal = self._pals[pal_id]
            existing_pal.update_from(pal)

        logger.info("Updated %d pals in the save file.", len(modified_pals))

        await ws_callback("Saving changes to file")

    async def update_dps_pals(
        self, modified_pals: Dict[int, PalDTO], ws_callback
    ) -> None:
        for pal_idx, pal in modified_pals.items():
            pal_name = pal.nickname if pal.nickname else pal.character_id
            await ws_callback(f"Updating DPS pal {pal_name}")
            player = self._players.get(pal.owner_uid)
            player.update_dps_pal(pal_idx, pal)

        logger.info("Updated %d pals in the save file.", len(modified_pals))

        await ws_callback("Saving changes to file")

    async def update_gps_pals(
        self, modified_pals: Dict[int, PalDTO], ws_callback
    ) -> None:
        if not self._gps_pals:
            raise ValueError("No GPS pals to update.")

        for pal_idx, pal in modified_pals.items():
            pal_name = pal.nickname if pal.nickname else pal.character_id
            await ws_callback(f"Updating GPS pal {pal_name}")
            if pal_idx not in self._gps_pals:
                logger.error("GPS pal index %d not found in the save file.", pal_idx)
                continue
            existing_pal = self._gps_pals[pal_idx]
            existing_pal.update_from(pal)

    def delete_gps_pals(self, pal_indexes: List[int]) -> None:
        if not self._gps_pals:
            logger.warning("No GPS pals to delete.")
            return
        for pal_idx in sorted(pal_indexes, reverse=True):
            if pal_idx in self._gps_pals:
                logger.debug("Deleting GPS pal at index %d", pal_idx)
                pal = self._gps_pals[pal_idx]
                pal.reset()
                del self._gps_pals[pal_idx]

    def _delete_pal_by_id(self, pal_id: UUID) -> None:
        del self._pals[pal_id]
        character_params = self._get_character_save_parameters()
        if character_params.remove_by_key(pal_id):
            logger.debug("Deleting pal %s from CharacterSaveParameterMap", pal_id)
            self.invalidate_performance_caches()
