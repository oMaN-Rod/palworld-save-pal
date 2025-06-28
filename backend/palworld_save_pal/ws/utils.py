from typing import Any
import uuid
from fastapi.encoders import jsonable_encoder

from shared.models import MessageType


def custom_jsonable_encoder(obj):
    """Recursively encode objects, converting UUIDs to strings."""
    if isinstance(obj, uuid.UUID):
        return str(obj)
    elif isinstance(obj, dict):
        return {k: custom_jsonable_encoder(v) for k, v in obj.items()}
    elif isinstance(obj, list):
        return [custom_jsonable_encoder(item) for item in obj]
    elif isinstance(obj, tuple):
        return tuple(custom_jsonable_encoder(item) for item in obj)
    elif isinstance(obj, set):
        return {custom_jsonable_encoder(item) for item in obj}
    else:
        # Try using standard jsonable_encoder for other types
        try:
            return jsonable_encoder(obj)
        except:
            # If it fails, try to convert to string
            return str(obj)


def build_response(message_type: MessageType, data: Any = None):
    return jsonable_encoder({"type": message_type.value, "data": data})


def build_response_custom(message_type: MessageType, data: Any = None):
    return custom_jsonable_encoder({"type": message_type.value, "data": data})
