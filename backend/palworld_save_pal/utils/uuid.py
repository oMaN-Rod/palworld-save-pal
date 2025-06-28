from typing import Any
import uuid


def is_valid_uuid(uuid_test: Any) -> bool:
    try:
        uuid.UUID(str(uuid_test))
        return True
    except ValueError:
        return False


def is_empty_uuid(uuid_test: Any) -> bool:
    return str(uuid_test) == "00000000-0000-0000-0000-000000000000"


def are_equal_uuids(uuid1: Any, uuid2: Any) -> bool:
    return str(uuid1).lower() == str(uuid2).lower()
