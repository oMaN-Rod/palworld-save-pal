from typing import TYPE_CHECKING, Dict, List, Optional, Tuple
from uuid import UUID

if TYPE_CHECKING:
    from palworld_save_pal.game.mixins._save_manager_protocol import (
        SaveManagerProtocol,
    )

    _Base = SaveManagerProtocol
else:
    _Base = object

from palworld_save_pal.dto.player import PlayerDTO
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.uuid import are_equal_uuids

logger = create_logger(__name__)


class PlayerOpsMixin(_Base):
    def get_players(self):
        return self._players

    def get_player(self, player_id: UUID):
        return self._players.get(player_id)

    async def delete_player(self, player_id: UUID, ws_callback) -> bool:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        player_guild = self._player_guild(player_id)
        if player_guild and are_equal_uuids(player_guild.admin_player_uid, player_id):
            logger.warning(
                "Cannot delete admin player %s from guild %s",
                player_id,
                player_guild.id,
            )
            return False
        elif player_guild:
            logger.debug(
                "Deleting player %s (%s) from guild %s (%s)",
                player.nickname,
                player.uid,
                player_guild.name,
                player_guild.id,
            )
            await ws_callback(
                f"Deleting player {player.nickname} from guild {player_guild.name}"
            )
            player_guild.delete_player(player_id)

        (
            container_ids_to_delete,
            character_container_ids_to_delete,
        ) = await self._delete_player_and_pals(player_id, ws_callback)

        # Delete all map objects owned by guild or player in guild
        await ws_callback(f"Deleting map objects of player {player.nickname}")
        self._map_object_save_data["values"][:] = [
            obj
            for obj in self._map_object_save_data["values"]
            if not self._should_delete_map_object(obj, None, [player_id])
        ]

        # Delete player items
        await ws_callback(f"Deleting item containers of player {player.nickname}")
        self._delete_item_containers(player_id, container_ids_to_delete)

        # Delete character containers
        await ws_callback(f"Deleting character containers of player {player.nickname}")
        self._delete_character_containers(character_container_ids_to_delete)
        return True

    async def _delete_player_and_pals(
        self, player_id: UUID, ws_callback
    ) -> Tuple[List[UUID], List[UUID]] | None:
        player = self._players[player_id]
        logger.debug("Deleting player %s with %s pals", player_id, len(player.pals))
        await ws_callback(
            f"Deleting player {player.nickname} with {len(player.pals)} pals"
        )

        # Container ids to delete
        container_ids_to_delete = [
            player.common_container.id,
            player.essential_container.id,
            player.weapon_load_out_container.id,
            player.player_equipment_armor_container.id,
            player.food_equip_container.id,
        ]

        # Character container ids to delete
        character_container_ids_to_delete = [
            player.otomo_container_id,
            player.pal_box_id,
        ]

        await ws_callback(
            f"Deleting {len(player.pal_box.slots)} pals of player {player.nickname} from PalBox"
        )
        for pal_slot in list(player.pal_box.slots):
            self._delete_pal_by_id(pal_slot.pal_id)

        await ws_callback(
            f"Deleting {len(player.party.slots)} pals of player {player.nickname} from Party"
        )
        for pal_slot in list(player.party.slots):
            self._delete_pal_by_id(pal_slot.pal_id)

        # Delete the player
        del self._players[player_id]

        # Delete player parameters
        self._character_save_parameter_map[:] = [
            entry
            for entry in self._character_save_parameter_map
            if not are_equal_uuids(
                PalObjects.get_guid(PalObjects.get_nested(entry, "key", "PlayerUId")),
                player_id,
            )
        ]
        self.invalidate_performance_caches()

        # Delete player save file
        del self._player_gvas_files[player_id]

        return container_ids_to_delete, character_container_ids_to_delete

    async def update_players(
        self, modified_players: Dict[UUID, PlayerDTO], ws_callback
    ) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for _, player in modified_players.items():
            await ws_callback(f"Updating player {player.nickname}")
            existing_player = self._players.get(player.uid)
            existing_player.update_from(player)

        logger.info("Updated %d players in the save file.", len(modified_players))

    async def update_player_technologies(
        self,
        player_id: UUID,
        technologies: Optional[list[str]] = None,
        technology_points: Optional[int] = None,
        boss_technology_points: Optional[int] = None,
        ws_callback=None,
    ) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        if technologies is not None:
            player.technologies = technologies
        if technology_points is not None:
            player.technology_points = technology_points
        if boss_technology_points is not None:
            player.boss_technology_points = boss_technology_points

        if ws_callback:
            await ws_callback("Updating player technologies and points")
