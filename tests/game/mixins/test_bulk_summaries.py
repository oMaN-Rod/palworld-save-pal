from datetime import datetime

from palworld_save_pal.game.mixins.summaries import ticks_to_datetime


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
    # At least one player in world1 has a real last-online timestamp
    assert any(s.last_online_time is not None for s in summaries.values())
