from uuid import UUID, uuid4

import pytest

from palworld_save_pal.game.character_container import (
    CharacterContainer,
    CharacterContainerSlot,
    CharacterContainerType,
)
from palworld_save_pal.game.pal_objects import PalObjects


TEST_UUID = UUID("12345678-1234-1234-1234-123456789abc")
CONTAINER_ID = UUID("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
PLAYER_UID = UUID("11111111-2222-3333-4444-555555555555")


def _make_container_data(slot_num=10, slots=None):
    """Build a minimal container_data dict matching the expected GVAS structure."""
    slot_data_list = []
    if slots:
        for slot_idx, instance_id in slots:
            slot_data_list.append(
                PalObjects.ContainerSlotData(slot_idx=slot_idx, instance_id=instance_id)
            )

    return {
        "value": {
            "SlotNum": PalObjects.IntProperty(slot_num),
            "Slots": PalObjects.ArrayPropertyValues(
                PalObjects.ArrayProperty.__func__.__code__ and __import__("palworld_save_pal.game.enum", fromlist=["ArrayType"]).ArrayType.STRUCT_PROPERTY
                if False
                else __import__("palworld_save_pal.game.enum", fromlist=["ArrayType"]).ArrayType.STRUCT_PROPERTY,
                slot_data_list,
            ),
        }
    }


def _simple_container_data(slot_num=10, slots=None):
    """Simplified container data builder."""
    from palworld_save_pal.game.enum import ArrayType

    slot_data_list = []
    if slots:
        for slot_idx, instance_id in slots:
            slot_data_list.append(
                PalObjects.ContainerSlotData(slot_idx=slot_idx, instance_id=instance_id)
            )

    return {
        "value": {
            "SlotNum": PalObjects.IntProperty(slot_num),
            "Slots": PalObjects.ArrayPropertyValues(
                ArrayType.STRUCT_PROPERTY,
                slot_data_list,
            ),
        }
    }


class TestCharacterContainerSlot:
    def test_create(self):
        slot = CharacterContainerSlot(slot_index=0, pal_id=TEST_UUID)
        assert slot.slot_index == 0
        assert slot.pal_id == TEST_UUID

    def test_none_pal_id(self):
        slot = CharacterContainerSlot(slot_index=5)
        assert slot.pal_id is None


class TestCharacterContainerType:
    def test_values(self):
        assert CharacterContainerType.PAL_BOX == "PalBox"
        assert CharacterContainerType.PARTY == "Party"
        assert CharacterContainerType.BASE == "Base"


class TestCharacterContainerNoData:
    def test_create_empty(self):
        cc = CharacterContainer(
            id=CONTAINER_ID,
            player_uid=PLAYER_UID,
            type=CharacterContainerType.PAL_BOX,
            size=480,
        )
        assert cc.id == CONTAINER_ID
        assert cc.size == 480
        assert cc.slots == []

    def test_available_slots_empty(self):
        cc = CharacterContainer(
            id=CONTAINER_ID,
            player_uid=PLAYER_UID,
            type=CharacterContainerType.PAL_BOX,
            size=5,
        )
        assert cc.available_slots() is True

    def test_add_pal(self):
        cc = CharacterContainer(
            id=CONTAINER_ID,
            player_uid=PLAYER_UID,
            type=CharacterContainerType.PAL_BOX,
            size=5,
        )
        pal_id = uuid4()
        slot_idx = cc.add_pal(pal_id)
        assert slot_idx == 0
        assert len(cc.slots) == 1
        assert cc.slots[0].pal_id == pal_id

    def test_add_pal_specific_slot(self):
        cc = CharacterContainer(
            id=CONTAINER_ID,
            player_uid=PLAYER_UID,
            type=CharacterContainerType.PARTY,
            size=5,
        )
        pal_id = uuid4()
        slot_idx = cc.add_pal(pal_id, storage_slot=3)
        assert slot_idx == 3

    def test_add_pal_full_returns_none(self):
        cc = CharacterContainer(
            id=CONTAINER_ID,
            player_uid=PLAYER_UID,
            type=CharacterContainerType.PARTY,
            size=1,
        )
        cc.add_pal(uuid4())
        result = cc.add_pal(uuid4())
        assert result is None

    def test_remove_pal(self):
        cc = CharacterContainer(
            id=CONTAINER_ID,
            player_uid=PLAYER_UID,
            type=CharacterContainerType.PAL_BOX,
            size=5,
        )
        pal_id = uuid4()
        cc.add_pal(pal_id)
        assert len(cc.slots) == 1
        cc.remove_pal(pal_id)
        assert len(cc.slots) == 0

    def test_find_first_available_slot(self):
        cc = CharacterContainer(
            id=CONTAINER_ID,
            player_uid=PLAYER_UID,
            type=CharacterContainerType.PAL_BOX,
            size=5,
        )
        cc.add_pal(uuid4(), storage_slot=0)
        cc.add_pal(uuid4(), storage_slot=1)
        cc.add_pal(uuid4(), storage_slot=3)
        assert cc.find_first_available_slot() == 2

    def test_find_last_available_slot(self):
        cc = CharacterContainer(
            id=CONTAINER_ID,
            player_uid=PLAYER_UID,
            type=CharacterContainerType.PAL_BOX,
            size=5,
        )
        cc.add_pal(uuid4(), storage_slot=0)
        cc.add_pal(uuid4(), storage_slot=4)
        assert cc.find_last_available_slot() == 3


class TestCharacterContainerWithData:
    def test_load_from_container_data(self):
        pal1 = uuid4()
        pal2 = uuid4()
        data = _simple_container_data(
            slot_num=10, slots=[(0, pal1), (1, pal2)]
        )
        cc = CharacterContainer(
            id=CONTAINER_ID,
            player_uid=PLAYER_UID,
            type=CharacterContainerType.PAL_BOX,
            container_data=data,
        )
        assert cc.size == 10
        assert len(cc.slots) == 2
        assert cc.slots[0].slot_index == 0
        assert cc.slots[1].slot_index == 1
