"""Extended integration tests for PalOpsMixin using real save data."""

from unittest.mock import MagicMock
from uuid import UUID, uuid4

import pytest

from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.game.pal import Pal
from palworld_save_pal.game.pal_objects import PalGender
from tests.game.conftest import GPS_FILE, PLAYER_O_UID, PLAYER_SKY_UID, _noop


PAL_BOX_ID = UUID("a6cb3db4-4760-f87f-c404-8b87887c0f29")


@pytest.fixture
def sm_with_player(event_loop, fresh_save_manager):
    event_loop.run_until_complete(
        fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
    )
    return fresh_save_manager


@pytest.fixture
def sm_with_gps(fresh_save_manager):
    with open(GPS_FILE, "rb") as f:
        fresh_save_manager.load_gps(f.read())
    return fresh_save_manager


def _make_pal_dto(pal: Pal) -> PalDTO:
    """Helper to create a PalDTO from an existing Pal."""
    return PalDTO(
        instance_id=pal.instance_id,
        owner_uid=pal.owner_uid,
        character_id=pal.character_id,
        is_lucky=pal.is_lucky,
        is_boss=pal.is_boss,
        gender=pal.gender,
        rank_hp=pal.rank_hp,
        rank_attack=pal.rank_attack,
        rank_defense=pal.rank_defense,
        rank_craftspeed=pal.rank_craftspeed,
        talent_hp=pal.talent_hp,
        talent_shot=pal.talent_shot,
        talent_defense=pal.talent_defense,
        rank=pal.rank,
        level=pal.level,
        exp=pal.exp,
        nickname=pal.nickname or pal.character_id,
        is_tower=pal.is_tower,
        storage_id=pal.storage_id,
        stomach=pal.stomach,
        storage_slot=pal.storage_slot,
        learned_skills=pal.learned_skills,
        active_skills=pal.active_skills,
        passive_skills=pal.passive_skills,
        hp=pal.hp,
        max_hp=pal.max_hp,
        group_id=pal.group_id,
        sanity=pal.sanity,
        work_suitability=pal.work_suitability,
        is_sick=pal.is_sick,
        friendship_point=pal.friendship_point,
    )


# ---------------------------------------------------------------------------
# get_pal / get_pals / pal_count error and edge cases
# ---------------------------------------------------------------------------
class TestGetPalEdgeCases:
    def test_get_pal_returns_none_for_random_uuid(self, sm_with_player):
        assert sm_with_player.get_pal(uuid4()) is None

    def test_get_pal_returns_none_for_empty_uuid(self, sm_with_player):
        empty = UUID("00000000-0000-0000-0000-000000000000")
        assert sm_with_player.get_pal(empty) is None

    def test_get_pals_returns_dict(self, sm_with_player):
        pals = sm_with_player.get_pals()
        assert isinstance(pals, dict)
        for pid, pal in pals.items():
            assert isinstance(pid, UUID)
            assert isinstance(pal, Pal)

    def test_pal_count_matches_get_pals_length(self, sm_with_player):
        assert sm_with_player.pal_count() == len(sm_with_player.get_pals())

    def test_pal_count_zero_before_player_load(self, fresh_save_manager):
        assert fresh_save_manager.pal_count() == 0

    def test_get_pals_empty_before_player_load(self, fresh_save_manager):
        pals = fresh_save_manager.get_pals()
        assert len(pals) == 0

    def test_get_pal_valid_id_returns_pal(self, sm_with_player):
        pal_id = next(iter(sm_with_player._pals.keys()))
        pal = sm_with_player.get_pal(pal_id)
        assert pal is not None
        assert pal.instance_id == pal_id
        assert pal.character_id is not None
        assert pal.level >= 1


# ---------------------------------------------------------------------------
# GPS pal operations
# ---------------------------------------------------------------------------
class TestAddGpsPal:
    def test_add_gps_pal_basic(self, sm_with_gps):
        initial_count = len(sm_with_gps._gps_pals)
        result = sm_with_gps.add_gps_pal(
            character_id="Lambball",
            nickname="GPSLamb",
        )
        assert result is not None
        pal, slot_idx = result
        assert isinstance(pal, Pal)
        assert pal.character_id == "Lambball"
        assert pal.nickname == "GPSLamb"
        assert slot_idx in sm_with_gps._gps_pals

    def test_add_gps_pal_with_specific_slot(self, sm_with_gps):
        # Find an empty slot by picking a slot that isn't occupied
        occupied = set(sm_with_gps._gps_pals.keys())
        # Use a slot index that we know exists but is empty
        target_slot = sm_with_gps._find_first_empty_gps_slot()
        if target_slot is None:
            pytest.skip("No empty GPS slots available")

        result = sm_with_gps.add_gps_pal(
            character_id="Cattiva",
            nickname="GPSCat",
            storage_slot=target_slot,
        )
        assert result is not None
        pal, slot_idx = result
        assert slot_idx == target_slot
        assert pal.character_id == "Cattiva"

    def test_add_gps_pal_without_gps_loaded_raises(self, fresh_save_manager):
        with pytest.raises(ValueError, match="GPS Gvas file is not initialized"):
            fresh_save_manager.add_gps_pal(
                character_id="Lambball",
                nickname="Test",
            )


class TestCloneGpsPal:
    def test_clone_gps_pal_adds_new_pal(self, sm_with_gps):
        source_idx = next(iter(sm_with_gps._gps_pals.keys()))
        source = sm_with_gps._gps_pals[source_idx]
        initial_count = len(sm_with_gps._gps_pals)

        result = sm_with_gps.clone_gps_pal(_make_pal_dto(source))

        assert result is not None
        slot_idx, new_pal = result
        assert isinstance(new_pal, Pal)
        assert len(sm_with_gps._gps_pals) == initial_count + 1
        assert sm_with_gps._gps_pals[slot_idx] is new_pal

    def test_clone_gps_pal_gets_new_instance_id(self, sm_with_gps):
        source_idx = next(iter(sm_with_gps._gps_pals.keys()))
        source = sm_with_gps._gps_pals[source_idx]

        slot_idx, new_pal = sm_with_gps.clone_gps_pal(_make_pal_dto(source))

        assert new_pal.instance_id != source.instance_id
        assert slot_idx != source_idx

    def test_clone_gps_pal_preserves_character(self, sm_with_gps):
        source_idx = next(iter(sm_with_gps._gps_pals.keys()))
        source = sm_with_gps._gps_pals[source_idx]

        _, new_pal = sm_with_gps.clone_gps_pal(_make_pal_dto(source))

        assert new_pal.character_id == source.character_id

    def test_clone_gps_pal_without_gps_loaded_raises(self, fresh_save_manager):
        # The GPS-loaded guard must fire before the DTO is ever read.
        with pytest.raises(ValueError, match="GPS Gvas file is not initialized"):
            fresh_save_manager.clone_gps_pal(MagicMock())


class TestDeleteGpsPals:
    def test_delete_single_gps_pal(self, sm_with_gps):
        initial_count = len(sm_with_gps._gps_pals)
        first_idx = next(iter(sm_with_gps._gps_pals.keys()))
        sm_with_gps.delete_gps_pals([first_idx])
        assert len(sm_with_gps._gps_pals) == initial_count - 1
        assert first_idx not in sm_with_gps._gps_pals

    def test_delete_multiple_gps_pals(self, sm_with_gps):
        indexes = list(sm_with_gps._gps_pals.keys())[:2]
        initial_count = len(sm_with_gps._gps_pals)
        sm_with_gps.delete_gps_pals(indexes)
        assert len(sm_with_gps._gps_pals) == initial_count - 2

    def test_delete_nonexistent_gps_pal_is_noop(self, sm_with_gps):
        initial_count = len(sm_with_gps._gps_pals)
        sm_with_gps.delete_gps_pals([9999])
        assert len(sm_with_gps._gps_pals) == initial_count

    def test_delete_gps_pals_without_gps_loaded(self, fresh_save_manager):
        # Should not raise, just warn
        fresh_save_manager.delete_gps_pals([0, 1])


# ---------------------------------------------------------------------------
# heal_all_player_pals
# ---------------------------------------------------------------------------
class TestHealAllPlayerPals:
    def test_heal_all_player_pals(self, sm_with_player):
        sm_with_player.heal_all_player_pals(PLAYER_O_UID)
        player = sm_with_player._players[PLAYER_O_UID]
        for pal in player.pals.values():
            assert pal.sanity == 100.0
            assert pal.is_sick is False

    def test_heal_all_player_pals_missing_player(self, sm_with_player):
        with pytest.raises(ValueError, match="not found"):
            sm_with_player.heal_all_player_pals(uuid4())


# ---------------------------------------------------------------------------
# move_pal
# ---------------------------------------------------------------------------
class TestMovePal:
    def test_move_pal_missing_player_raises(self, sm_with_player):
        pal_id = next(iter(sm_with_player._pals.keys()))
        with pytest.raises(ValueError, match="not found"):
            sm_with_player.move_pal(uuid4(), pal_id, PAL_BOX_ID)

    def test_move_pal_to_same_container(self, sm_with_player):
        pal_id = next(iter(sm_with_player._pals.keys()))
        pal = sm_with_player._pals[pal_id]
        original_storage = pal.storage_id
        sm_with_player.move_pal(PLAYER_O_UID, pal_id, original_storage)
        assert pal.storage_id == original_storage
        assert pal_id in sm_with_player._pals


# ---------------------------------------------------------------------------
# update_pals (async)
# ---------------------------------------------------------------------------
class TestUpdatePals:
    def test_update_pals_changes_nickname(self, event_loop, sm_with_player):
        pal_id = next(iter(sm_with_player._pals.keys()))
        pal = sm_with_player._pals[pal_id]
        dto = _make_pal_dto(pal)
        dto.nickname = "UpdatedName"

        event_loop.run_until_complete(
            sm_with_player.update_pals({pal_id: dto}, ws_callback=_noop)
        )
        updated = sm_with_player.get_pal(pal_id)
        assert updated.nickname == "UpdatedName"

    def test_update_pals_changes_level(self, event_loop, sm_with_player):
        pal_id = next(iter(sm_with_player._pals.keys()))
        pal = sm_with_player._pals[pal_id]
        dto = _make_pal_dto(pal)
        dto.level = 50

        event_loop.run_until_complete(
            sm_with_player.update_pals({pal_id: dto}, ws_callback=_noop)
        )
        updated = sm_with_player.get_pal(pal_id)
        assert updated.level == 50

    def test_update_pals_empty_dict_is_noop(self, event_loop, sm_with_player):
        event_loop.run_until_complete(
            sm_with_player.update_pals({}, ws_callback=_noop)
        )
        assert sm_with_player.pal_count() == 11

    def test_update_pals_no_gvas_raises(self, event_loop, fresh_save_manager):
        fresh_save_manager._gvas_file = None
        with pytest.raises(ValueError, match="No GvasFile"):
            event_loop.run_until_complete(
                fresh_save_manager.update_pals({uuid4(): None}, ws_callback=_noop)
            )


# ---------------------------------------------------------------------------
# _delete_pal_by_id
# ---------------------------------------------------------------------------
class TestDeletePalById:
    def test_delete_pal_by_id_removes_from_pals(self, sm_with_player):
        pal_id = next(iter(sm_with_player._pals.keys()))
        initial_count = sm_with_player.pal_count()
        sm_with_player._delete_pal_by_id(pal_id)
        assert sm_with_player.pal_count() == initial_count - 1
        assert sm_with_player.get_pal(pal_id) is None

    def test_delete_pal_by_id_nonexistent_raises(self, sm_with_player):
        with pytest.raises(KeyError):
            sm_with_player._delete_pal_by_id(uuid4())
