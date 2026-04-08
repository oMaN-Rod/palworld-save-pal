from uuid import UUID, uuid4

import pytest
from pydantic import ValidationError

from palworld_save_pal.dto.dynamic_item import DynamicItemDTO
from palworld_save_pal.dto.item_container_slot import ItemContainerSlotDTO
from palworld_save_pal.dto.settings import SettingsDTO
from palworld_save_pal.dto.summary import GuildSummary, PlayerSummary
from palworld_save_pal.game.enum import PalGender


TEST_UUID = UUID("12345678-1234-1234-1234-123456789abc")


class TestItemContainerSlotDTO:
    def test_create(self):
        slot = ItemContainerSlotDTO(
            slot_index=0, count=5, static_id="Sword_001"
        )
        assert slot.slot_index == 0
        assert slot.count == 5
        assert slot.static_id == "Sword_001"
        assert slot.dynamic_item is None

    def test_with_dynamic_item(self):
        di = DynamicItemDTO(local_id=TEST_UUID)
        slot = ItemContainerSlotDTO(
            slot_index=1, count=1, static_id="Bow_001", dynamic_item=di
        )
        assert slot.dynamic_item.local_id == TEST_UUID


class TestDynamicItemDTO:
    def test_defaults(self):
        di = DynamicItemDTO(local_id=TEST_UUID)
        assert di.local_id == TEST_UUID
        assert di.modified is False
        assert di.durability is None
        assert di.type is None

    def test_with_values(self):
        di = DynamicItemDTO(
            local_id=TEST_UUID,
            durability=85.5,
            type="weapon",
            character_id="Lambball",
        )
        assert di.durability == pytest.approx(85.5)
        assert di.type == "weapon"


class TestSettingsDTO:
    def test_create(self):
        s = SettingsDTO(
            language="en",
            clone_prefix="C",
            new_pal_prefix="N",
            debug_mode=False,
            cheat_mode=True,
        )
        assert s.language == "en"
        assert s.cheat_mode is True


class TestPlayerSummary:
    def test_create(self):
        ps = PlayerSummary(
            uid=TEST_UUID,
            nickname="TestPlayer",
            level=25,
            guild_id=None,
            pal_count=10,
            loaded=False,
        )
        assert ps.uid == TEST_UUID
        assert ps.nickname == "TestPlayer"
        assert ps.level == 25
        assert ps.loaded is False


class TestGuildSummary:
    def test_create(self):
        gs = GuildSummary(
            id=TEST_UUID,
            name="TestGuild",
            admin_player_uid=TEST_UUID,
            player_count=3,
            base_count=2,
            loaded=False,
        )
        assert gs.name == "TestGuild"
        assert gs.player_count == 3


# ---------------------------------------------------------------------------
# Validation error tests — verify Pydantic rejects bad input
# ---------------------------------------------------------------------------


class TestDTOValidation:
    def test_item_container_slot_rejects_bad_slot_index_type(self):
        with pytest.raises(ValidationError):
            ItemContainerSlotDTO(slot_index="not_an_int", count=1)

    def test_dynamic_item_rejects_bad_uuid(self):
        with pytest.raises(ValidationError):
            DynamicItemDTO(local_id="not-a-uuid")

    def test_settings_dto_missing_required_language(self):
        with pytest.raises(ValidationError):
            SettingsDTO(
                clone_prefix="C",
                new_pal_prefix="N",
                debug_mode=False,
                cheat_mode=False,
            )

    def test_player_summary_rejects_bad_uid(self):
        with pytest.raises(ValidationError):
            PlayerSummary(
                uid="not-a-uuid",
                nickname="Test",
                level=1,
                pal_count=0,
                loaded=False,
            )

    def test_guild_summary_missing_required_name(self):
        with pytest.raises(ValidationError):
            GuildSummary(
                id=TEST_UUID,
                admin_player_uid=TEST_UUID,
                player_count=1,
                base_count=0,
                loaded=False,
            )
