"""Tests for player_swap mixin - pure helpers and integration."""

from uuid import UUID, uuid4

import pytest

from palworld_save_pal.game.mixins.player_swap import (
    _deep_swap_uids,
    _swap_uid_value,
)
from tests.game.conftest import PLAYER_O_UID, PLAYER_SKY_UID, _noop


# ---------------------------------------------------------------------------
# Pure helper: _swap_uid_value
# ---------------------------------------------------------------------------


class TestSwapUidValue:
    def test_match_old_returns_new(self):
        assert _swap_uid_value("aaa", "aaa", "bbb") == "bbb"

    def test_match_new_returns_old(self):
        assert _swap_uid_value("bbb", "aaa", "bbb") == "aaa"

    def test_no_match_returns_none(self):
        assert _swap_uid_value("ccc", "aaa", "bbb") is None

    def test_case_insensitive(self):
        assert _swap_uid_value("AAA", "aaa", "bbb") == "bbb"


# ---------------------------------------------------------------------------
# Pure helper: _deep_swap_uids
# ---------------------------------------------------------------------------


class TestDeepSwapUids:
    def test_swaps_owner_player_uid_string(self):
        data = {"owner_player_uid": "aaa"}
        _deep_swap_uids(data, "aaa", "bbb")
        assert data["owner_player_uid"] == "bbb"

    def test_swaps_owner_player_uid_dict_value(self):
        data = {"OwnerPlayerUId": {"value": "aaa"}}
        _deep_swap_uids(data, "aaa", "bbb")
        assert data["OwnerPlayerUId"]["value"] == "bbb"

    def test_swaps_build_player_uid(self):
        data = {"build_player_uid": "bbb"}
        _deep_swap_uids(data, "aaa", "bbb")
        assert data["build_player_uid"] == "aaa"

    def test_swaps_private_lock_player_uid(self):
        data = {"private_lock_player_uid": "aaa"}
        _deep_swap_uids(data, "aaa", "bbb")
        assert data["private_lock_player_uid"] == "bbb"

    def test_no_match_leaves_unchanged(self):
        data = {"owner_player_uid": "ccc"}
        _deep_swap_uids(data, "aaa", "bbb")
        assert data["owner_player_uid"] == "ccc"

    def test_nested_dict(self):
        data = {"outer": {"inner": {"owner_player_uid": "aaa"}}}
        _deep_swap_uids(data, "aaa", "bbb")
        assert data["outer"]["inner"]["owner_player_uid"] == "bbb"

    def test_list_of_dicts(self):
        data = [{"owner_player_uid": "aaa"}, {"owner_player_uid": "bbb"}]
        _deep_swap_uids(data, "aaa", "bbb")
        assert data[0]["owner_player_uid"] == "bbb"
        assert data[1]["owner_player_uid"] == "aaa"

    def test_ignores_non_ownership_keys(self):
        data = {"some_other_key": "aaa"}
        _deep_swap_uids(data, "aaa", "bbb")
        assert data["some_other_key"] == "aaa"

    def test_none_value_skipped(self):
        data = {"owner_player_uid": None}
        _deep_swap_uids(data, "aaa", "bbb")
        assert data["owner_player_uid"] is None


# ---------------------------------------------------------------------------
# Integration: swap_player_uids
# ---------------------------------------------------------------------------


class TestSwapPlayerUidsIntegration:
    @pytest.fixture
    def sm_with_both(self, event_loop, fresh_save_manager):
        event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )
        event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_SKY_UID, ws_callback=_noop)
        )
        return fresh_save_manager

    def test_same_uid_returns_error(self, event_loop, sm_with_both):
        result = event_loop.run_until_complete(
            sm_with_both.swap_player_uids(PLAYER_O_UID, PLAYER_O_UID, ws_callback=_noop)
        )
        assert "error" in result

    def test_missing_player_returns_error(self, event_loop, sm_with_both):
        fake_uid = uuid4()
        result = event_loop.run_until_complete(
            sm_with_both.swap_player_uids(PLAYER_O_UID, fake_uid, ws_callback=_noop)
        )
        assert "error" in result

    def test_successful_swap(self, event_loop, sm_with_both):
        # Both players are level >= 2 (O=65, Sky=2)
        result = event_loop.run_until_complete(
            sm_with_both.swap_player_uids(
                PLAYER_O_UID, PLAYER_SKY_UID, ws_callback=_noop
            )
        )
        assert result.get("success") is True

    def test_swap_updates_summaries(self, event_loop, sm_with_both):
        event_loop.run_until_complete(
            sm_with_both.swap_player_uids(
                PLAYER_O_UID, PLAYER_SKY_UID, ws_callback=_noop
            )
        )
        summaries = sm_with_both.get_player_summaries()
        # After swap, the summaries should be rebuilt
        assert PLAYER_O_UID in summaries
        assert PLAYER_SKY_UID in summaries

    def test_swap_clears_loaded_players(self, event_loop, sm_with_both):
        assert sm_with_both.is_player_loaded(PLAYER_O_UID) is True
        event_loop.run_until_complete(
            sm_with_both.swap_player_uids(
                PLAYER_O_UID, PLAYER_SKY_UID, ws_callback=_noop
            )
        )
        # rebuild_player_caches clears loaded state
        assert sm_with_both.is_player_loaded(PLAYER_O_UID) is False


# ---------------------------------------------------------------------------
# rebuild_player_caches
# ---------------------------------------------------------------------------


class TestRebuildPlayerCaches:
    def test_clears_and_rebuilds(self, event_loop, fresh_save_manager):
        event_loop.run_until_complete(
            fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )
        assert len(fresh_save_manager._players) == 1

        fresh_save_manager.rebuild_player_caches()

        assert len(fresh_save_manager._players) == 0
        assert len(fresh_save_manager._loaded_players) == 0
        # Summaries should be rebuilt
        assert len(fresh_save_manager.get_player_summaries()) == 2