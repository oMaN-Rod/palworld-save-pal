from uuid import UUID, uuid4

from palworld_save_pal.ws.messages import MessageType
from palworld_save_pal.ws.utils import build_response, build_response_custom, custom_jsonable_encoder


class TestBuildResponse:
    def test_basic(self):
        resp = build_response(MessageType.ERROR, "something went wrong")
        assert resp["type"] == "error"
        assert resp["data"] == "something went wrong"

    def test_none_data(self):
        resp = build_response(MessageType.GET_SETTINGS)
        assert resp["type"] == "get_settings"
        assert resp["data"] is None

    def test_dict_data(self):
        resp = build_response(MessageType.PROGRESS_MESSAGE, {"progress": 50})
        assert resp["data"]["progress"] == 50


class TestBuildResponseCustom:
    def test_uuid_converted_to_string(self):
        uid = uuid4()
        resp = build_response_custom(MessageType.ERROR, {"id": uid})
        assert resp["data"]["id"] == str(uid)


class TestCustomJsonableEncoder:
    def test_uuid(self):
        uid = uuid4()
        assert custom_jsonable_encoder(uid) == str(uid)

    def test_dict_with_uuids(self):
        uid = uuid4()
        result = custom_jsonable_encoder({"id": uid, "name": "test"})
        assert result["id"] == str(uid)
        assert result["name"] == "test"

    def test_list_with_uuids(self):
        uid = uuid4()
        result = custom_jsonable_encoder([uid, "hello"])
        assert result[0] == str(uid)
        assert result[1] == "hello"

    def test_nested(self):
        uid = uuid4()
        result = custom_jsonable_encoder({"items": [{"id": uid}]})
        assert result["items"][0]["id"] == str(uid)

    def test_primitive(self):
        assert custom_jsonable_encoder(42) == 42
        assert custom_jsonable_encoder("hello") == "hello"
