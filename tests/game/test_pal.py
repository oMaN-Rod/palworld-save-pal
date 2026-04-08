"""Integration tests for Pal entity using real save data."""

import asyncio
from uuid import UUID

import pytest

from palworld_save_pal.game.pal import Pal
from palworld_save_pal.game.enum import PalGender
from tests.game.conftest import PLAYER_O_UID, _noop


@pytest.fixture
def loaded_pals(event_loop, fresh_save_manager):
    event_loop.run_until_complete(
        fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
    )
    return fresh_save_manager._pals


class TestPalProperties:
    def test_pals_loaded(self, loaded_pals):
        assert len(loaded_pals) == 11

    def test_pal_has_character_id(self, loaded_pals):
        for pal in loaded_pals.values():
            assert pal.character_id is not None
            assert isinstance(pal.character_id, str)
            assert len(pal.character_id) > 0

    def test_pal_has_level(self, loaded_pals):
        for pal in loaded_pals.values():
            assert pal.level >= 1

    def test_pal_has_gender(self, loaded_pals):
        for pal in loaded_pals.values():
            assert isinstance(pal.gender, PalGender)

    def test_pal_has_owner(self, loaded_pals):
        for pal in loaded_pals.values():
            assert pal.owner_uid == PLAYER_O_UID

    def test_pal_has_instance_id(self, loaded_pals):
        instance_ids = set()
        for pal in loaded_pals.values():
            assert pal.instance_id is not None
            instance_ids.add(pal.instance_id)
        assert len(instance_ids) == len(loaded_pals)

    def test_pal_has_storage_info(self, loaded_pals):
        for pal in loaded_pals.values():
            assert pal.storage_id is not None
            assert pal.storage_slot is not None

    def test_pal_has_hp(self, loaded_pals):
        for pal in loaded_pals.values():
            assert pal.hp is not None
            assert pal.max_hp is not None
            assert pal.max_hp > 0

    def test_pal_has_talents(self, loaded_pals):
        for pal in loaded_pals.values():
            assert 0 <= pal.talent_hp <= 100
            assert 0 <= pal.talent_shot <= 100
            assert 0 <= pal.talent_defense <= 100

    def test_pal_has_work_suitability(self, loaded_pals):
        for pal in loaded_pals.values():
            assert isinstance(pal.work_suitability, dict)

    def test_specific_pal_jetdragon(self, loaded_pals):
        jet_dragons = [p for p in loaded_pals.values() if p.character_id == "JetDragon"]
        assert len(jet_dragons) >= 1
        jd = jet_dragons[0]
        assert jd.level == 65


class TestPalBooleanProperties:
    def test_is_lucky(self, loaded_pals):
        for pal in loaded_pals.values():
            assert isinstance(pal.is_lucky, bool)

    def test_is_boss(self, loaded_pals):
        for pal in loaded_pals.values():
            assert isinstance(pal.is_boss, bool)

    def test_is_tower(self, loaded_pals):
        for pal in loaded_pals.values():
            assert isinstance(pal.is_tower, bool)

    def test_is_predator(self, loaded_pals):
        for pal in loaded_pals.values():
            assert isinstance(pal.is_predator, bool)

    def test_is_sick(self, loaded_pals):
        for pal in loaded_pals.values():
            assert isinstance(pal.is_sick, bool)


class TestPalModification:
    def test_set_level(self, loaded_pals):
        pal = next(iter(loaded_pals.values()))
        original = pal.level
        pal.level = 50
        assert pal.level == 50
        pal.level = original

    def test_set_nickname(self, loaded_pals):
        pal = next(iter(loaded_pals.values()))
        pal.nickname = "TestNickname"
        assert pal.nickname == "TestNickname"
