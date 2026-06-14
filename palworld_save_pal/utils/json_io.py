import base64
import math
import uuid
from typing import Any, Optional

import orjson

from palworld_save_tools.archive import UUID as ArchiveUUID


def _default(obj: Any) -> Any:
    if isinstance(obj, (ArchiveUUID, uuid.UUID)):
        return str(obj)
    if isinstance(obj, (bytes, bytearray)):
        return base64.b64encode(bytes(obj)).decode("ascii")
    raise TypeError(f"Object of type {type(obj).__name__} is not JSON serializable")


def _sanitize_nonfinite(obj: Any) -> Any:
    if isinstance(obj, float):
        if math.isnan(obj) or math.isinf(obj):
            return None
        return obj
    if isinstance(obj, dict):
        return {k: _sanitize_nonfinite(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [_sanitize_nonfinite(v) for v in obj]
    if isinstance(obj, tuple):
        return tuple(_sanitize_nonfinite(v) for v in obj)
    return obj


def _build_option(indent: Optional[int], sort_keys: bool, non_str_keys: bool) -> int:
    option = 0
    if non_str_keys:
        option |= orjson.OPT_NON_STR_KEYS
    if sort_keys:
        option |= orjson.OPT_SORT_KEYS
    if indent == 2:
        option |= orjson.OPT_INDENT_2
    return option


def dumps(
    data: Any,
    *,
    indent: Optional[int] = None,
    sort_keys: bool = False,
    allow_nan: bool = True,
    non_str_keys: bool = True,
) -> bytes:
    if not allow_nan:
        data = _sanitize_nonfinite(data)
    option = _build_option(indent, sort_keys, non_str_keys)
    return orjson.dumps(data, default=_default, option=option)


def dumps_str(
    data: Any,
    *,
    indent: Optional[int] = None,
    sort_keys: bool = False,
    allow_nan: bool = True,
    non_str_keys: bool = True,
) -> str:
    return dumps(
        data,
        indent=indent,
        sort_keys=sort_keys,
        allow_nan=allow_nan,
        non_str_keys=non_str_keys,
    ).decode("utf-8")


def loads(data: Any) -> Any:
    return orjson.loads(data)


def dump(
    data: Any,
    path: str,
    *,
    indent: Optional[int] = None,
    sort_keys: bool = False,
    allow_nan: bool = True,
    non_str_keys: bool = True,
) -> None:
    buf = dumps(
        data,
        indent=indent,
        sort_keys=sort_keys,
        allow_nan=allow_nan,
        non_str_keys=non_str_keys,
    )
    with open(path, "wb") as f:
        f.write(buf)


def load(path: str) -> Any:
    with open(path, "rb") as f:
        return orjson.loads(f.read())
