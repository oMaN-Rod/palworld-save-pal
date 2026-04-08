"""Tests for ItemContainerSlot model."""

from uuid import UUID

import pytest

from palworld_save_pal.game.item_container_slot import ItemContainerSlot
from palworld_save_pal.game.pal_objects import PalObjects


TEST_UUID = UUID("12345678-1234-1234-1234-123456789abc")
EMPTY_UUID = UUID("00000000-0000-0000-0000-000000000000")


def _make_container_slot_data(
    slot_index=0,
    count=1,
    static_id="Weapon_AssaultRifle_Default1",
    local_id=None,
):
    return {
        "RawData": {
            "value": {
                "slot_index": slot_index,
                "count": count,
                "item": {
                    "static_id": static_id,
                    "dynamic_id": {
                        "local_id_in_created_world": local_id
                        if local_id
                        else str(EMPTY_UUID),
                    },
                },
            },
        },
    }


class TestItemContainerSlotConstruction:
    def test_create_from_data(self):
        data = _make_container_slot_data()
        slot = ItemContainerSlot(container_slot_data=data)
        assert slot is not None

    def test_slot_data_property(self):
        data = _make_container_slot_data()
        slot = ItemContainerSlot(container_slot_data=data)
        assert slot.slot_data is data


class TestItemContainerSlotComputedProperties:
    @pytest.fixture
    def slot(self):
        data = _make_container_slot_data(
            slot_index=3,
            count=5,
            static_id="Apple",
            local_id=str(TEST_UUID),
        )
        return ItemContainerSlot(container_slot_data=data)

    def test_slot_index(self, slot):
        assert slot.slot_index == 3

    def test_count(self, slot):
        assert slot.count == 5

    def test_static_id(self, slot):
        assert slot.static_id == "Apple"

    def test_local_id(self, slot):
        assert slot.local_id == TEST_UUID


class TestItemContainerSlotSetters:
    @pytest.fixture
    def slot(self):
        data = _make_container_slot_data(
            slot_index=0, count=1, static_id="Stone"
        )
        return ItemContainerSlot(container_slot_data=data)

    def test_set_slot_index(self, slot):
        slot.slot_index = 7
        assert slot.slot_index == 7

    def test_set_count(self, slot):
        slot.count = 99
        assert slot.count == 99

    def test_set_static_id(self, slot):
        slot.static_id = "Wood"
        assert slot.static_id == "Wood"

    def test_set_local_id(self, slot):
        new_id = str(TEST_UUID)
        slot.local_id = new_id
        # The raw data stores the string; the computed field converts via as_uuid
        assert slot.local_id == TEST_UUID


class TestItemContainerSlotUpdateFrom:
    @pytest.fixture
    def slot(self):
        data = _make_container_slot_data(
            slot_index=0, count=1, static_id="Stone"
        )
        return ItemContainerSlot(container_slot_data=data)

    def test_update_slot_index(self, slot):
        slot.update_from({"slot_index": 5})
        assert slot.slot_index == 5

    def test_update_count(self, slot):
        slot.update_from({"count": 42})
        assert slot.count == 42

    def test_update_static_id(self, slot):
        slot.update_from({"static_id": "Iron"})
        assert slot.static_id == "Iron"

    def test_update_skips_dynamic_item(self, slot):
        slot.update_from({"dynamic_item": "should_be_ignored", "count": 10})
        assert slot.count == 10
        assert slot.dynamic_item is None

    def test_update_multiple_fields(self, slot):
        slot.update_from({"slot_index": 2, "count": 50, "static_id": "Gold"})
        assert slot.slot_index == 2
        assert slot.count == 50
        assert slot.static_id == "Gold"

    def test_update_ignores_unknown_keys(self, slot):
        slot.update_from({"nonexistent_field": 123, "count": 7})
        assert slot.count == 7


class TestItemContainerSlotEmptyDynamic:
    def test_local_id_empty_uuid(self):
        data = _make_container_slot_data(local_id=str(EMPTY_UUID))
        slot = ItemContainerSlot(container_slot_data=data)
        assert slot.local_id == EMPTY_UUID

    def test_dynamic_item_default_none(self):
        data = _make_container_slot_data()
        slot = ItemContainerSlot(container_slot_data=data)
        assert slot.dynamic_item is None
