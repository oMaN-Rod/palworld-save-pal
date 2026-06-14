"""Integration tests for PalOpsMixin using real save data."""

from uuid import UUID, uuid4

import pytest

from palworld_save_pal.game.pal import Pal
from tests.game.conftest import PLAYER_O_UID, _noop


@pytest.fixture
def sm_with_player(event_loop, fresh_save_manager):
    event_loop.run_until_complete(
        fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
    )
    return fresh_save_manager


class TestGetPal:
    def test_get_pal(self, sm_with_player):
        pal_id = next(iter(sm_with_player._pals.keys()))
        pal = sm_with_player.get_pal(pal_id)
        assert pal is not None
        assert isinstance(pal, Pal)

    def test_get_pal_missing(self, sm_with_player):
        assert sm_with_player.get_pal(uuid4()) is None

    def test_get_pals(self, sm_with_player):
        pals = sm_with_player.get_pals()
        assert len(pals) == 11

    def test_pal_count(self, sm_with_player):
        assert sm_with_player.pal_count() == 11


class TestAddPlayerPal:
    def test_add_pal_to_player(self, sm_with_player):
        initial_count = len(sm_with_player._pals)
        player = sm_with_player._players[PLAYER_O_UID]
        container_id = player.pal_box_id

        result = sm_with_player.add_player_pal(
            player_id=PLAYER_O_UID,
            character_id="Lambball",
            nickname="TestAdd",
            container_id=container_id,
        )
        assert result is not None
        assert isinstance(result, Pal)
        assert result.character_id == "Lambball"
        assert len(sm_with_player._pals) == initial_count + 1


class TestClonePal:
    def test_clone_pal(self, sm_with_player):
        pal = next(iter(sm_with_player._pals.values()))
        initial_count = len(sm_with_player._pals)

        from palworld_save_pal.dto.pal import PalDTO

        pal_dto = PalDTO(
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
        result = sm_with_player.clone_pal(pal=pal_dto)
        assert result is not None
        assert result.instance_id != pal.instance_id
        assert len(sm_with_player._pals) == initial_count + 1


class TestDeletePals:
    def test_delete_player_pals(self, sm_with_player):
        pal_ids = list(sm_with_player._pals.keys())[:2]
        initial_count = len(sm_with_player._pals)

        sm_with_player.delete_player_pals(
            player_id=PLAYER_O_UID,
            pal_ids=pal_ids,
        )
        assert len(sm_with_player._pals) == initial_count - 2


class TestHealPals:
    def test_heal_pals(self, sm_with_player):
        pal_ids = list(sm_with_player._pals.keys())[:2]
        sm_with_player.heal_pals(pal_ids=pal_ids)
        for pid in pal_ids:
            pal = sm_with_player._pals.get(pid)
            if pal:
                # heal() restores sanity and stomach, removes sickness
                assert pal.sanity == 100.0
                assert pal.is_sick is False
