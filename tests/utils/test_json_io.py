import base64
import math
import uuid

import pytest

from palworld_save_tools.archive import UUID as ArchiveUUID

from palworld_save_pal.utils import json_io


def test_roundtrip_primitives(tmp_path):
    data = {"a": 1, "b": "hello", "c": [1, 2, 3], "d": True, "e": None}
    path = tmp_path / "x.json"
    json_io.dump(data, str(path))
    assert json_io.load(str(path)) == data


def test_bytes_encoded_as_base64():
    payload = b"\x00\x01\x02\xffhello"
    out = json_io.dumps({"v": payload})
    loaded = json_io.loads(out)
    assert base64.b64decode(loaded["v"]) == payload


def test_uuid_encoded_as_string():
    u = uuid.uuid4()
    out = json_io.dumps({"u": u})
    assert json_io.loads(out)["u"] == str(u)


def test_archive_uuid_encoded_as_string():
    u = ArchiveUUID.from_str("00000000-0000-0000-0000-000000000001")
    out = json_io.dumps({"u": u})
    assert json_io.loads(out)["u"] == str(u)


def test_allow_nan_false_sanitizes():
    data = {"nan": float("nan"), "inf": float("inf"), "ok": 1.5}
    out = json_io.dumps(data, allow_nan=False)
    loaded = json_io.loads(out)
    assert loaded["nan"] is None
    assert loaded["inf"] is None
    assert loaded["ok"] == 1.5


def test_indent_two_produces_pretty():
    out = json_io.dumps({"a": 1}, indent=2)
    assert b"\n" in out


def test_non_str_keys_supported():
    out = json_io.dumps({1: "one"})
    assert json_io.loads(out) == {"1": "one"}


def test_dumps_str_returns_utf8():
    s = json_io.dumps_str({"k": "🙂"})
    assert isinstance(s, str)
    assert "🙂" in s


def test_default_raises_for_unknown():
    class Weird:
        pass

    with pytest.raises(TypeError):
        json_io.dumps({"x": Weird()})
