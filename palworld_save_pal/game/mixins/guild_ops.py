from typing import TYPE_CHECKING, Any, Dict, List, Optional
from uuid import UUID

if TYPE_CHECKING:
    from palworld_save_pal.game.mixins._save_manager_protocol import (
        SaveManagerProtocol,
    )

    _Base = SaveManagerProtocol
else:
    _Base = object

from palworld_save_pal.dto.guild import GuildDTO
from palworld_save_pal.game.guild import Guild
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.uuid import are_equal_uuids, is_empty_uuid

logger = create_logger(__name__)


class GuildOpsMixin(_Base):
    def get_guild(self, guild_id: UUID) -> Optional[Guild]:
        return self._guilds.get(guild_id, None)

    def get_guilds(self):
        return self._guilds

    def get_base(self, base_id: UUID):
        for guild in self._guilds.values():
            base = guild.bases.get(base_id)
            if base:
                return base
        return None

    async def delete_guild_and_players(self, guild_id: UUID, ws_callback) -> None:
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Guild {guild_id} not found in the save file.")
        logger.debug("Deleting guild %s with %s players", guild_id, len(guild.players))
        await ws_callback(
            f"Deleting guild {guild.name} with {len(guild.players)} players"
        )
        # Container ids to delete
        container_ids_to_delete = []

        # Character container ids to delete
        character_container_ids_to_delete = []

        # Delete all map objects owned by guild or player in guild
        self._map_object_save_data["values"][:] = [
            obj
            for obj in self._map_object_save_data["values"]
            if not self._should_delete_map_object(obj, guild_id, guild.players)
        ]

        # Delete all players in the guild
        for player_id in guild.players:
            if player_id not in self._players:
                continue
            container_ids, character_container_ids = await self._delete_player_and_pals(
                player_id, ws_callback
            )
            container_ids_to_delete.extend(container_ids)
            character_container_ids_to_delete.extend(character_container_ids)

        # Remove guild extra save data
        self._guild_extra_save_data_map[:] = [
            entry
            for entry in self._guild_extra_save_data_map
            if not are_equal_uuids(entry["key"], guild_id)
        ]

        # Delete all bases in the guild
        for base_id, base in guild.bases.items():
            logger.debug("Deleting base %s", base_id)
            await ws_callback(f"Deleting base {base.id}")
            container_ids_to_delete.extend(list(base.storage_containers.keys()))
            character_container_ids_to_delete.append(base.container_id)

            self.delete_guild_pals(guild_id, base_id, list(base.pals.keys()))

            self._base_camp_save_data_map[:] = [
                base
                for base in self._base_camp_save_data_map
                if not are_equal_uuids(PalObjects.get_nested(base, "key"), base_id)
            ]

        # Delete player items and guild items
        await ws_callback(f"Deleting item containers of guild {guild.name}")
        self._delete_item_containers(guild_id, container_ids_to_delete)
        self._delete_item_containers(player_id, container_ids_to_delete)

        # Delete character containers
        await ws_callback(f"Deleting character containers of guild {guild.name}")
        self._delete_character_containers(character_container_ids_to_delete)

        # Delete the guild
        self._group_save_data_map[:] = [
            group
            for group in self._group_save_data_map
            if not are_equal_uuids(PalObjects.get_nested(group, "key"), guild_id)
        ]
        del self._guilds[guild_id]

    async def update_guilds(
        self, modified_guilds: Dict[UUID, GuildDTO], ws_callback
    ) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for id, dto in modified_guilds.items():
            logger.debug("Updating guild %s", id)
            await ws_callback(f"Updating guild {id}")
            guild = self._guilds.get(id)
            guild.update_from(dto)

        logger.info("Updated %d bases in the save file.", len(modified_guilds))

    def _should_delete_map_object(
        self, map_object: dict, guild_id: UUID | None, player_ids: List[UUID]
    ) -> bool:
        raw_data = map_object["Model"]["value"]["RawData"]["value"]
        group_id = PalObjects.as_uuid(raw_data.get("group_id_belong_to"))
        build_player_uid = PalObjects.as_uuid(raw_data.get("build_player_uid"))

        # Check guild ownership
        if guild_id and are_equal_uuids(group_id, guild_id):
            return True

        # Check if any player in the list is the builder
        if any(
            are_equal_uuids(build_player_uid, player_id) for player_id in player_ids
        ):
            return True

        # Handle edge cases
        if "ConcreteModel" in map_object:
            concrete_model_raw_data = map_object["ConcreteModel"]["value"]["RawData"][
                "value"
            ]
            private_lock_player_uid = PalObjects.as_uuid(
                concrete_model_raw_data.get("private_lock_player_uid")
            )

            # Check if any player in the list is the private lock owner
            if any(
                are_equal_uuids(private_lock_player_uid, player_id)
                for player_id in player_ids
            ):
                return True

            # Check trade info sellers
            for trade_info in concrete_model_raw_data.get("trade_infos", []):
                seller_player_uid = PalObjects.as_uuid(
                    trade_info.get("seller_player_uid")
                )
                if any(
                    are_equal_uuids(seller_player_uid, player_id)
                    for player_id in player_ids
                ):
                    return True

            # Check password lock module
            for module in concrete_model_raw_data.get("ModuleMap", {}).get("value", []):
                if (
                    module["key"]
                    == "EPalMapObjectConcreteModelModuleType::PasswordLock"
                ):
                    for player_info in module["value"]["RawData"]["value"].get(
                        "player_infos", []
                    ):
                        player_uid = PalObjects.as_uuid(player_info.get("player_uid"))
                        if any(
                            are_equal_uuids(player_uid, player_id)
                            for player_id in player_ids
                        ):
                            return True

        return False

    def _delete_item_containers(
        self, target_id: UUID, container_ids_to_delete: List[UUID]
    ) -> None:
        logger.debug(
            "Deleting %s item containers for %s",
            len(container_ids_to_delete),
            target_id,
        )
        item_containers = self._get_item_containers()
        for container_id in container_ids_to_delete:
            entry = item_containers.get(container_id)
            if entry:
                self._delete_dynamic_items(entry)
                item_containers.remove_by_key(container_id)
            else:
                # Fallback: search by GroupId if not found by container_id
                for entry in item_containers.data:
                    if are_equal_uuids(
                        PalObjects.get_guid(
                            PalObjects.get_nested(
                                entry, "value", "BelongInfo", "value", "GroupId"
                            )
                        ),
                        target_id,
                    ):
                        self._delete_dynamic_items(entry)
                        item_containers.remove(entry)
                        break

    def _delete_dynamic_items(self, item_container: UUID) -> None:
        slots = PalObjects.get_array_property(
            PalObjects.get_nested(item_container, "value", "Slots")
        )
        dynamic_items = self._get_dynamic_items()
        for slot in slots:
            raw_data = PalObjects.get_value(slot["RawData"])
            local_id = PalObjects.as_uuid(
                PalObjects.get_nested(
                    raw_data, "item", "dynamic_id", "local_id_in_created_world"
                )
            )
            if local_id and not is_empty_uuid(local_id):
                logger.debug("Deleting dynamic item %s", local_id)
                if dynamic_items.remove_by_key(local_id):
                    logger.debug("Deleted dynamic item %s", local_id)

    def _delete_character_containers(self, container_ids_to_delete: List[UUID]) -> None:
        logger.debug("Deleting character containers for %s", container_ids_to_delete)
        character_containers = self._get_character_containers()
        for container_id in container_ids_to_delete:
            character_containers.remove_by_key(container_id)