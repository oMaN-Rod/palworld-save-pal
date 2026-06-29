from datetime import datetime

from palworld_save_pal.game.mixins.summaries import ticks_to_datetime
from tests.game.conftest import _load_save_manager, WORLD1_DIR


def test_ticks_to_datetime_epoch_is_year_one():
    assert ticks_to_datetime(0) == datetime(1, 1, 1)


def test_ticks_to_datetime_one_day():
    # 1 day = 86400 s * 10_000_000 ticks/s
    assert ticks_to_datetime(86400 * 10_000_000) == datetime(1, 1, 2)


def test_player_summaries_include_last_online_time(fresh_save_manager):
    summaries = fresh_save_manager.get_player_summaries()
    assert len(summaries) > 0
    for summary in summaries.values():
        # Field exists and is either a datetime or None
        assert hasattr(summary, "last_online_time")
        assert summary.last_online_time is None or isinstance(
            summary.last_online_time, datetime
        )

    # Build a separate local manager (not the shared fixture) to prove the
    # extraction path populates a real datetime when data IS present.
    sm = _load_save_manager(WORLD1_DIR)
    injected_count = 0
    for entry in sm._character_save_parameter_map:
        try:
            save_parameter = entry["value"]["RawData"]["value"]["object"]["SaveParameter"]["value"]
            if sm._is_player(entry) and injected_count == 0:
                save_parameter["LastOnlineRealTime"] = {
                    "type": "UInt64Property",
                    "value": 1_000_000_000_000,  # 1 trillion ticks
                    "id": None,
                }
                injected_count += 1
        except (KeyError, TypeError):
            continue
    sm._extract_player_summaries()
    injected_summaries = sm.get_player_summaries()
    assert any(
        isinstance(s.last_online_time, datetime)
        for s in injected_summaries.values()
    )


def test_guild_summaries_include_level_and_pal_count(fresh_save_manager):
    summaries = fresh_save_manager.get_guild_summaries()
    assert len(summaries) > 0
    for summary in summaries.values():
        assert hasattr(summary, "level")
        assert hasattr(summary, "pal_count")
        assert isinstance(summary.pal_count, int)
        assert summary.pal_count >= 0

    # The world1 guild that has at least one base should report a level.
    guild_with_base = next(
        (s for s in summaries.values() if s.base_count > 0), None
    )
    assert guild_with_base is not None
    assert guild_with_base.level is not None
    assert guild_with_base.pal_count >= 0
