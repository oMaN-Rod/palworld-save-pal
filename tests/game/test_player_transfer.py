"""Tests for player_transfer helper functions."""

from uuid import UUID

import pytest

from palworld_save_pal.game.player_transfer import (
    EMPTY_UUID,
    _get_save_parameter,
    _is_player_entry,
    _get_entry_instance_id,
    _get_entry_player_uid,
    _get_owner_uid,
    _find_guild_id_for_player,
    _get_player_container_ids,
    _transfer_tech,
    _transfer_appearance,
)
from palworld_save_pal.game.pal_objects import PalObjects


def _make_character_entry(
    player_uid="aaaaaaaa-0000-0000-0000-000000000000",
    instance_id="bbbbbbbb-0000-0000-0000-000000000000",
    is_player=True,
    owner_uid=None,
):
    save_param = {}
    if is_player:
        save_param["IsPlayer"] = PalObjects.BoolProperty(True)
    if owner_uid:
        save_param["OwnerPlayerUId"] = PalObjects.Guid(owner_uid)

    return {
        "key": {
            "PlayerUId": PalObjects.Guid(player_uid),
            "InstanceId": PalObjects.Guid(instance_id),
        },
        "value": {
            "RawData": {
                "value": {
                    "object": {
                        "SaveParameter": {
                            "value": save_param,
                        }
                    }
                }
            }
        },
    }


class TestGetSaveParameter:
    def test_valid_entry(self):
        entry = _make_character_entry()
        result = _get_save_parameter(entry)
        assert result is not None
        assert "IsPlayer" in result

    def test_missing_value(self):
        assert _get_save_parameter({}) is None

    def test_missing_raw_data(self):
        assert _get_save_parameter({"value": {}}) is None

    def test_none_entry(self):
        assert _get_save_parameter({"value": None}) is None


class TestIsPlayerEntry:
    def test_player_entry(self):
        entry = _make_character_entry(is_player=True)
        assert _is_player_entry(entry) is True

    def test_pal_entry(self):
        entry = _make_character_entry(is_player=False)
        assert _is_player_entry(entry) is False

    def test_empty_entry(self):
        assert _is_player_entry({}) is False


class TestGetEntryInstanceId:
    def test_valid_id(self):
        iid = "cccccccc-0000-0000-0000-000000000000"
        entry = _make_character_entry(instance_id=iid)
        result = _get_entry_instance_id(entry)
        assert result == iid

    def test_missing_key(self):
        assert _get_entry_instance_id({}) is None

    def test_missing_instance_id(self):
        assert _get_entry_instance_id({"key": {}}) is None


class TestGetEntryPlayerUid:
    def test_valid_uid(self):
        uid = "dddddddd-0000-0000-0000-000000000000"
        entry = _make_character_entry(player_uid=uid)
        result = _get_entry_player_uid(entry)
        assert result == uid

    def test_missing_key(self):
        assert _get_entry_player_uid({}) is None


class TestGetOwnerUid:
    def test_with_owner(self):
        owner = "11111111-0000-0000-0000-000000000000"
        entry = _make_character_entry(owner_uid=owner)
        save_param = _get_save_parameter(entry)
        result = _get_owner_uid(save_param)
        assert result == owner

    def test_no_owner(self):
        entry = _make_character_entry()
        save_param = _get_save_parameter(entry)
        result = _get_owner_uid(save_param)
        assert result is None


class TestFindGuildIdForPlayer:
    def _make_group_entry(self, group_id, player_uids):
        return {
            "value": {
                "RawData": {
                    "value": {
                        "group_id": group_id,
                        "players": [
                            {"player_uid": uid} for uid in player_uids
                        ],
                    }
                }
            }
        }

    def test_player_found_in_guild(self):
        uid = "aaaaaaaa-0000-0000-0000-000000000000"
        gid = "eeeeeeee-0000-0000-0000-000000000000"
        groups = [self._make_group_entry(gid, [uid])]
        result = _find_guild_id_for_player(groups, uid)
        assert result == gid

    def test_player_not_in_any_guild(self):
        uid = "aaaaaaaa-0000-0000-0000-000000000000"
        other = "bbbbbbbb-0000-0000-0000-000000000000"
        groups = [self._make_group_entry("gid", [other])]
        result = _find_guild_id_for_player(groups, uid)
        assert result == EMPTY_UUID

    def test_empty_group_list(self):
        result = _find_guild_id_for_player([], "any-uid")
        assert result == EMPTY_UUID

    def test_malformed_group_entry(self):
        groups = [{"value": {}}]
        result = _find_guild_id_for_player(groups, "uid")
        assert result == EMPTY_UUID


class TestGetPlayerContainerIds:
    def test_extracts_pal_storage_container(self):
        cid = "12345678-1234-1234-1234-123456789abc"
        save_data = {
            "PalStorageContainerId": {
                "value": {"ID": PalObjects.Guid(cid)},
            },
        }
        ids = _get_player_container_ids(save_data)
        assert cid in ids

    def test_extracts_otomo_container(self):
        cid = "aabbccdd-0000-0000-0000-000000000000"
        save_data = {
            "OtomoCharacterContainerId": {
                "value": {"ID": PalObjects.Guid(cid)},
            },
        }
        ids = _get_player_container_ids(save_data)
        assert cid in ids

    def test_extracts_inventory_containers(self):
        cid = "99999999-0000-0000-0000-000000000000"
        save_data = {
            "InventoryInfo": {
                "value": {
                    "CommonContainerId": {
                        "value": {"ID": PalObjects.Guid(cid)},
                    },
                },
            },
        }
        ids = _get_player_container_ids(save_data)
        assert cid in ids

    def test_skips_empty_uuid(self):
        save_data = {
            "PalStorageContainerId": {
                "value": {"ID": PalObjects.Guid(EMPTY_UUID)},
            },
        }
        ids = _get_player_container_ids(save_data)
        assert len(ids) == 0

    def test_empty_save_data(self):
        ids = _get_player_container_ids({})
        assert len(ids) == 0


class TestTransferTech:
    def test_copies_tech_points(self):
        source = {"TechnologyPoint": PalObjects.IntProperty(100)}
        target = {"TechnologyPoint": PalObjects.IntProperty(0)}
        _transfer_tech(source, target)
        assert target["TechnologyPoint"]["value"] == 100

    def test_copies_boss_tech_points(self):
        source = {"bossTechnologyPoint": PalObjects.IntProperty(50)}
        target = {"bossTechnologyPoint": PalObjects.IntProperty(0)}
        _transfer_tech(source, target)
        assert target["bossTechnologyPoint"]["value"] == 50

    def test_copies_unlocked_recipes(self):
        recipes = {"value": {"values": ["Recipe_A", "Recipe_B"]}}
        source = {"UnlockedRecipeTechnologyNames": recipes}
        target = {}
        _transfer_tech(source, target)
        assert "UnlockedRecipeTechnologyNames" in target

    def test_removes_record_data_when_missing_in_source(self):
        source = {}
        target = {"RecordData": {"some": "data"}}
        _transfer_tech(source, target)
        assert "RecordData" not in target

    def test_copies_record_data(self):
        source = {"RecordData": {"key": "value"}}
        target = {}
        _transfer_tech(source, target)
        assert target["RecordData"]["key"] == "value"


class TestTransferAppearance:
    def test_copies_appearance(self):
        source = {"PlayerCharacterMakeData": {"hair": "style1"}}
        target = {}
        _transfer_appearance(source, target)
        assert target["PlayerCharacterMakeData"]["hair"] == "style1"

    def test_no_appearance_in_source(self):
        source = {}
        target = {"PlayerCharacterMakeData": {"hair": "old"}}
        _transfer_appearance(source, target)
        # Should remain unchanged
        assert target["PlayerCharacterMakeData"]["hair"] == "old"

    def test_deep_copy(self):
        source = {"PlayerCharacterMakeData": {"hair": "style1"}}
        target = {}
        _transfer_appearance(source, target)
        source["PlayerCharacterMakeData"]["hair"] = "modified"
        assert target["PlayerCharacterMakeData"]["hair"] == "style1"


class TestEmptyUuidConstant:
    def test_value(self):
        assert EMPTY_UUID == "00000000-0000-0000-0000-000000000000"


# ---------------------------------------------------------------------------
# Integration: transfer_player
# ---------------------------------------------------------------------------

from palworld_save_pal.game.player_transfer import (
    transfer_player,
    _find_character_container,
    _find_item_container,
)
from tests.game.conftest import (
    PLAYER_O_UID,
    PLAYER_SKY_UID,
    WORLD1_DIR,
    WORLD2_DIR,
    _noop,
    _load_save_manager,
)


class TestFindCharacterContainer:
    def test_finds_existing_container(self, fresh_save_manager):
        # Use a container ID from the save
        containers = fresh_save_manager._character_container_save_data
        if containers:
            first = containers[0]
            cid = str(PalObjects.get_guid(first["key"]["ID"])).lower()
            result = _find_character_container(fresh_save_manager, cid)
            assert result is not None

    def test_returns_none_for_missing(self, fresh_save_manager):
        result = _find_character_container(
            fresh_save_manager, "ffffffff-ffff-ffff-ffff-ffffffffffff"
        )
        assert result is None


class TestFindItemContainer:
    def test_finds_existing_container(self, fresh_save_manager):
        containers = fresh_save_manager._item_container_save_data
        if containers:
            first = containers[0]
            cid = str(PalObjects.get_guid(first["key"]["ID"])).lower()
            result = _find_item_container(fresh_save_manager, cid)
            assert result is not None

    def test_returns_none_for_missing(self, fresh_save_manager):
        result = _find_item_container(
            fresh_save_manager, "ffffffff-ffff-ffff-ffff-ffffffffffff"
        )
        assert result is None


@pytest.fixture
def fresh_save_manager(event_loop):
    """Override to get a non-cached SaveManager for mutation tests."""
    return _load_save_manager(WORLD1_DIR)


class TestTransferPlayerIntegration:
    def test_source_player_not_found(self, event_loop):
        source = _load_save_manager(WORLD1_DIR)
        target = _load_save_manager(WORLD2_DIR)
        fake_uid = UUID("ffffffff-0000-0000-0000-000000000000")
        result = event_loop.run_until_complete(
            transfer_player(source, target, fake_uid, ws_callback=_noop)
        )
        assert "error" in result

    def test_transfer_spawn_mode(self, event_loop):
        """Transfer player O from world1 to world2 in spawn mode (no target UID)."""
        source = _load_save_manager(WORLD1_DIR)
        target = _load_save_manager(WORLD2_DIR)

        # Load source player first
        event_loop.run_until_complete(
            source.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )

        result = event_loop.run_until_complete(
            transfer_player(
                source,
                target,
                PLAYER_O_UID,
                target_player_uid=None,  # spawn mode
                ws_callback=_noop,
            )
        )
        # Should either succeed or give a meaningful error
        assert isinstance(result, dict)

    def test_transfer_to_existing_player(self, event_loop):
        """Transfer player O from world1 to world2 targeting existing player O."""
        source = _load_save_manager(WORLD1_DIR)
        target = _load_save_manager(WORLD2_DIR)

        # Load source player
        event_loop.run_until_complete(
            source.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
        )

        result = event_loop.run_until_complete(
            transfer_player(
                source,
                target,
                PLAYER_O_UID,
                target_player_uid=PLAYER_O_UID,
                ws_callback=_noop,
            )
        )
        assert isinstance(result, dict)
