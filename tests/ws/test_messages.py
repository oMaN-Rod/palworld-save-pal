from uuid import UUID, uuid4

import pytest

from palworld_save_pal.ws.messages import (
    AddGpsPalMessage,
    AddPalData,
    AddPalMessage,
    BaseMessage,
    ClonePalData,
    ClonePalMessage,
    DeleteGpsPalsMessage,
    DeletePalsData,
    DeletePalsMessage,
    MessageType,
    MovePalData,
    MovePalMessage,
)


class TestMessageType:
    def test_all_values_are_strings(self):
        for mt in MessageType:
            assert isinstance(mt.value, str)

    def test_unique_values(self):
        values = [mt.value for mt in MessageType]
        assert len(values) == len(set(values))

    def test_key_types_exist(self):
        assert MessageType.ADD_PAL.value == "add_pal"
        assert MessageType.DELETE_PALS.value == "delete_pals"
        assert MessageType.ERROR.value == "error"
        assert MessageType.GET_SETTINGS.value == "get_settings"


class TestBaseMessage:
    def test_create(self):
        msg = BaseMessage(type="test", data={"key": "value"})
        assert msg.type == "test"
        assert msg.data == {"key": "value"}

    def test_none_data(self):
        msg = BaseMessage(type="test")
        assert msg.data is None


class TestAddPalData:
    def test_create(self):
        uid = uuid4()
        data = AddPalData(
            player_id=uid,
            character_id="Lambball",
            nickname="TestPal",
        )
        assert data.player_id == uid
        assert data.character_id == "Lambball"
        assert data.container_id is None

    def test_all_optional_none(self):
        data = AddPalData(character_id="Cattiva", nickname="Cat")
        assert data.player_id is None
        assert data.guild_id is None
        assert data.base_id is None


class TestAddPalMessage:
    def test_create(self):
        msg = AddPalMessage(
            data=AddPalData(character_id="Lambball", nickname="Test")
        )
        assert msg.type == MessageType.ADD_PAL.value
        assert msg.data.character_id == "Lambball"


class TestMovePalData:
    def test_create(self):
        data = MovePalData(
            player_id=uuid4(),
            pal_id=uuid4(),
            container_id=uuid4(),
        )
        assert isinstance(data.player_id, UUID)


class TestDeletePalsData:
    def test_create(self):
        uid = uuid4()
        data = DeletePalsData(
            player_id=uid,
            pal_ids=[uuid4(), uuid4()],
        )
        assert data.player_id == uid
        assert len(data.pal_ids) == 2


class TestClonePalData:
    def test_create(self):
        from palworld_save_pal.dto.pal import PalDTO
        from palworld_save_pal.game.enum import PalGender

        pal = PalDTO(
            instance_id=uuid4(),
            owner_uid=uuid4(),
            character_id="Lambball",
            is_lucky=False,
            is_boss=False,
            gender=PalGender.FEMALE,
            rank_hp=0, rank_attack=0, rank_defense=0, rank_craftspeed=0,
            talent_hp=50, talent_shot=50, talent_defense=50,
            rank=1, level=10, exp=0,
            nickname="TestPal",
            is_tower=False,
            storage_id=uuid4(),
            stomach=300.0,
            storage_slot=0,
            learned_skills=[], active_skills=[], passive_skills=[],
            hp=5000, max_hp=5000,
            group_id=uuid4(),
            sanity=100.0,
            work_suitability={},
            is_sick=False,
            friendship_point=0,
        )
        data = ClonePalData(pal=pal)
        assert data.pal.character_id == "Lambball"


class TestGpsMessageDefaults:
    def test_add_gps_pal_message_default_type(self):
        msg = AddGpsPalMessage(
            data=AddPalData(character_id="Lambball", nickname="Test")
        )
        assert msg.type == MessageType.ADD_GPS_PAL.value

    def test_delete_gps_pals_message_default_type(self):
        msg = DeleteGpsPalsMessage(data=DeletePalsData(pal_indexes=[0]))
        assert msg.type == MessageType.DELETE_GPS_PALS.value
