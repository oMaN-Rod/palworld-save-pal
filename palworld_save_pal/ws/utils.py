from typing import Any

from palworld_save_pal.ws.messages import MessageType


def build_response(message_type: MessageType, data: Any = None):
    return {"type": message_type.value, "data": data}
