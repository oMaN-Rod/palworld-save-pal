"""Tests for pal summary extraction (bulk actions plan 3)."""

from uuid import uuid4

from palworld_save_pal.game.pal_objects import PalObjects
from tests.game.conftest import PLAYER_O_UID, WORLD1_DIR, _load_save_manager


def test_pal_summaries_cover_owned_pals():
    sm = _load_save_manager(WORLD1_DIR)
    pals = sm.get_pal_summaries()
    assert len(pals) > 0
    owned_by_o = [p for p in pals if p.owner_uid == PLAYER_O_UID]
    assert len(owned_by_o) == 11
    player_summary = sm.get_player_summaries()[PLAYER_O_UID]
    assert all(p.owner_name == player_summary.nickname for p in owned_by_o)


def test_pal_summary_core_fields_populated():
    sm = _load_save_manager(WORLD1_DIR)
    pal = next(p for p in sm.get_pal_summaries() if p.owner_uid == PLAYER_O_UID)
    assert pal.instance_id is not None
    assert pal.character_id
    assert pal.character_key
    assert pal.level >= 1


def test_injected_base_pal_maps_to_guild_and_base():
    sm = _load_save_manager(WORLD1_DIR)
    base = sm._base_camp_save_data_map[0]
    guild_id = PalObjects.as_uuid(
        PalObjects.get_nested(base, "value", "RawData", "value", "group_id_belong_to")
    )
    base_id = PalObjects.as_uuid(PalObjects.get_nested(base, "key"))
    container_id = PalObjects.as_uuid(
        PalObjects.get_nested(
            base, "value", "WorkerDirector", "value", "RawData", "value", "container_id"
        )
    )
    assert guild_id and base_id and container_id

    injected_id = uuid4()
    entry = {
        "key": {
            "PlayerUId": PalObjects.Guid("00000000-0000-0000-0000-000000000000"),
            "InstanceId": PalObjects.Guid(injected_id),
        },
        "value": {
            "RawData": {
                "value": {
                    "object": {
                        "SaveParameter": {
                            "value": {
                                "SlotId": PalObjects.PalCharacterSlotId(container_id, 99),
                                "CharacterID": PalObjects.StrProperty("SheepBall"),
                            }
                        }
                    }
                }
            }
        },
    }
    sm._character_save_parameter_map.append(entry)

    injected = next(p for p in sm.get_pal_summaries() if p.instance_id == injected_id)
    assert injected.guild_id == guild_id
    assert injected.base_id == base_id
    assert injected.owner_uid is None
    assert injected.owner_name is None
