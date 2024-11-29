from typing import Any
from fastapi.encoders import jsonable_encoder

from palworld_save_pal.ws.messages import MessageType


def build_response(message_type: MessageType, data: Any = None):
    return jsonable_encoder({"type": message_type.value, "data": data})
