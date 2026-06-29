from concurrent.futures import ThreadPoolExecutor, as_completed
import time
from datetime import datetime, timedelta
from typing import TYPE_CHECKING, Any, Dict, List, Optional, Tuple
from uuid import UUID

if TYPE_CHECKING:
    from palworld_save_pal.game.mixins._save_manager_protocol import (
        SaveManagerProtocol,
    )

    _Base = SaveManagerProtocol
else:
    _Base = object

from palworld_save_pal.dto.summary import PlayerSummary, GuildSummary
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.game.enum import GroupType
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.uuid import are_equal_uuids, is_empty_uuid
from palworld_save_pal.utils.json_manager import sanitize_string

logger = create_logger(__name__)


def ticks_to_datetime(ticks: int) -> datetime:
    """Convert .NET-style ticks (100ns since 0001-01-01) to a datetime."""
    seconds = ticks / 10_000_000
    days = int(seconds // 86400)
    seconds_remainder = seconds % 86400
    return datetime(1, 1, 1) + timedelta(days=days, seconds=seconds_remainder)


class SummariesMixin(_Base):
    def get_player_summaries(self) -> Dict[UUID, PlayerSummary]:
        valid_summaries = {}
        filtered_count = 0

        for player_id, summary in self._player_summaries.items():
            if player_id in self._player_file_refs:
                valid_summaries[player_id] = summary
            else:
                filtered_count += 1
                logger.warning(
                    f"Filtering out player {player_id} ({summary.nickname}) - no .sav file reference"
                )

        if filtered_count > 0:
            logger.info(
                f"Filtered {filtered_count} players without .sav files, "
                f"returning {len(valid_summaries)} valid players"
            )

        return valid_summaries

    def get_guild_summaries(self) -> Dict[UUID, GuildSummary]:
        return self._guild_summaries

    def _extract_player_summaries(self) -> Dict[UUID, PlayerSummary]:
        start_time = time.perf_counter()
        players_data, pal_owner_counts = self._categorize_character_entries()
        player_guild_map = self._get_player_guild_map()

        if len(players_data) > 2:
            summaries = self._extract_players_parallel(
                players_data, player_guild_map, pal_owner_counts
            )
        else:
            summaries = self._extract_players_sequential(
                players_data, player_guild_map, pal_owner_counts
            )

        self._player_summaries = summaries
        elapsed = time.perf_counter() - start_time
        logger.info(f"Extracted {len(summaries)} player summaries in {elapsed:.3f}s")
        return summaries

    def _categorize_character_entries(
        self,
    ) -> Tuple[List[Tuple[UUID, Dict[str, Any]]], Dict[UUID, int]]:
        players_data: List[Tuple[UUID, Dict[str, Any]]] = []
        pal_owner_counts: Dict[UUID, int] = {}

        for entry in self._character_save_parameter_map:
            try:
                save_parameter = entry["value"]["RawData"]["value"]["object"][
                    "SaveParameter"
                ]["value"]
            except (KeyError, TypeError):
                continue

            if self._is_player(entry):
                try:
                    uid = PalObjects.get_guid(entry["key"]["PlayerUId"])
                    if uid and not is_empty_uuid(uid):
                        players_data.append((uid, save_parameter))
                except (KeyError, TypeError):
                    continue
            else:
                owner_uid_data = save_parameter.get("OwnerPlayerUId")
                if owner_uid_data:
                    owner_uid = PalObjects.get_guid(owner_uid_data)
                    if owner_uid:
                        pal_owner_counts[owner_uid] = (
                            pal_owner_counts.get(owner_uid, 0) + 1
                        )

        self._pal_owner_counts_cache = pal_owner_counts

        return players_data, pal_owner_counts

    def _get_player_guild_map(self) -> Dict[UUID, UUID]:
        if self._player_guild_map_cache is not None:
            return self._player_guild_map_cache

        self._player_guild_map_cache = self._build_player_guild_index()
        return self._player_guild_map_cache

    def _build_player_guild_index(self) -> Dict[UUID, UUID]:
        player_guild_map: Dict[UUID, UUID] = {}

        if not self._group_save_data_map:
            return player_guild_map

        for entry in self._group_save_data_map:
            group_type = PalObjects.get_enum_property(
                PalObjects.get_nested(entry, "value", "GroupType")
            )
            if GroupType.from_value(group_type) != GroupType.GUILD:
                continue

            guild_id = PalObjects.as_uuid(PalObjects.get_nested(entry, "key"))
            if not guild_id:
                continue

            try:
                raw_data = entry["value"]["RawData"]["value"]
            except (KeyError, TypeError):
                continue

            for player_entry in raw_data.get("players", []):
                player_uid = PalObjects.as_uuid(player_entry.get("player_uid"))
                if player_uid:
                    player_guild_map[player_uid] = guild_id

        return player_guild_map

    def _create_player_summary(
        self,
        uid: UUID,
        save_parameter: Dict[str, Any],
        player_guild_map: Dict[UUID, UUID],
        pal_owner_counts: Dict[UUID, int],
    ) -> PlayerSummary:
        nickname = None
        if "NickName" in save_parameter:
            nickname_data = save_parameter["NickName"]
            if isinstance(nickname_data, dict):
                nickname = nickname_data.get("value")
            else:
                nickname = nickname_data
        if not nickname:
            nickname = f"Player ({str(uid)[:8]})"
        nickname = sanitize_string(nickname)

        level = None
        if "Level" in save_parameter:
            level = PalObjects.get_byte_property(save_parameter["Level"])

        last_online_time = None
        last_online_raw = save_parameter.get("LastOnlineRealTime")
        if last_online_raw is not None:
            ticks = PalObjects.get_value(last_online_raw)
            if ticks:
                last_online_time = ticks_to_datetime(ticks)

        return PlayerSummary(
            uid=uid,
            nickname=nickname,
            level=level,
            guild_id=player_guild_map.get(uid),
            pal_count=pal_owner_counts.get(uid, 0),
            last_online_time=last_online_time,
            loaded=False,
        )

    def _extract_players_sequential(
        self,
        players_data: List[Tuple[UUID, Dict[str, Any]]],
        player_guild_map: Dict[UUID, UUID],
        pal_owner_counts: Dict[UUID, int],
    ) -> Dict[UUID, PlayerSummary]:
        summaries = {}
        for uid, save_parameter in players_data:
            summaries[uid] = self._create_player_summary(
                uid, save_parameter, player_guild_map, pal_owner_counts
            )
            logger.debug(
                "Extracted player summary for player: %s", summaries[uid].nickname
            )
        return summaries

    def _extract_players_parallel(
        self,
        players_data: List[Tuple[UUID, Dict[str, Any]]],
        player_guild_map: Dict[UUID, UUID],
        pal_owner_counts: Dict[UUID, int],
    ) -> Dict[UUID, PlayerSummary]:
        summaries = {}
        max_workers = min(4, len(players_data))

        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            future_to_uid = {
                executor.submit(
                    self._create_player_summary,
                    uid,
                    save_parameter,
                    player_guild_map,
                    pal_owner_counts,
                ): uid
                for uid, save_parameter in players_data
            }

            for future in as_completed(future_to_uid):
                uid = future_to_uid[future]
                try:
                    summaries[uid] = future.result()
                    logger.debug(
                        "Extracted player summary for player: %s",
                        summaries[uid].nickname,
                    )
                except Exception as e:
                    logger.error(f"Error extracting player {uid}: {e}")

        return summaries

    def _extract_guild_summaries(self) -> Dict[UUID, GuildSummary]:
        summaries = {}

        if not self._group_save_data_map:
            self._guild_summaries = summaries
            return summaries

        for entry in self._group_save_data_map:
            group_type = PalObjects.get_enum_property(
                PalObjects.get_nested(entry, "value", "GroupType")
            )
            group_type = GroupType.from_value(group_type)
            if group_type != GroupType.GUILD:
                continue

            guild_id = PalObjects.as_uuid(PalObjects.get_nested(entry, "key"))
            if not guild_id or is_empty_uuid(guild_id):
                continue

            raw_data = PalObjects.get_nested(entry, "value", "RawData", "value")
            if not raw_data:
                continue

            name = raw_data.get("guild_name", "Unknown Guild")
            name = sanitize_string(name)
            players = raw_data.get("players", [])
            admin_player_uid = PalObjects.as_uuid(raw_data.get("admin_player_uid"))

            base_count = 0
            if self._base_camp_save_data_map:
                base_count = sum(
                    1
                    for base in self._base_camp_save_data_map
                    if are_equal_uuids(
                        PalObjects.as_uuid(
                            PalObjects.get_nested(
                                base, "value", "RawData", "value", "group_id_belong_to"
                            )
                        ),
                        guild_id,
                    )
                )

            summaries[guild_id] = GuildSummary(
                id=guild_id,
                name=name,
                admin_player_uid=admin_player_uid,
                player_count=len(players),
                base_count=base_count,
                loaded=False,
            )

        self._guild_summaries = summaries
        logger.info(f"Extracted {len(summaries)} guild summaries")
        return summaries