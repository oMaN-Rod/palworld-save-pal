from uuid import UUID, uuid4

import pytest

from palworld_save_pal.game.item_container import ItemContainer, ItemContainerType


TEST_UUID = UUID("12345678-1234-1234-1234-123456789abc")


class TestItemContainerType:
    def test_values(self):
        assert ItemContainerType.COMMON == "CommonContainer"
        assert ItemContainerType.ESSENTIAL == "EssentialContainer"
        assert ItemContainerType.WEAPON == "WeaponLoadOutContainer"
        assert ItemContainerType.ARMOR == "PlayerEquipArmorContainer"
        assert ItemContainerType.FOOD == "FoodEquipContainer"
        assert ItemContainerType.BASE == "BaseContainer"
        assert ItemContainerType.GUILD == "GuildChest"

    def test_all_types(self):
        assert len(ItemContainerType) == 7


class TestItemContainerNoData:
    def test_create_empty(self):
        ic = ItemContainer(
            id=TEST_UUID,
            type=ItemContainerType.COMMON,
        )
        assert ic.id == TEST_UUID
        assert ic.type == ItemContainerType.COMMON
        assert ic.slots == []
        assert ic.key is None

    def test_slot_num_no_data(self):
        ic = ItemContainer(id=TEST_UUID, type=ItemContainerType.COMMON)
        assert ic.slot_num == 0

    def test_with_key(self):
        ic = ItemContainer(id=TEST_UUID, type=ItemContainerType.BASE, key="base_storage_1")
        assert ic.key == "base_storage_1"
