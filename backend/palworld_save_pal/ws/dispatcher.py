import palworld_save_pal.ws.handlers.bootstrap as handlers
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class MessageDispatcher:
    def __init__(self):
        self.handlers = {}

    def register_handler(self, message_type, handler):
        self.handlers[message_type] = handler

    async def dispatch(self, message_data, websocket):
        message_type = message_data.get("type")
        handler = self.handlers.get(message_type)
        if handler:
            message = handler["message_class"](**message_data)
            response = await handler["handler_func"](message, websocket)
        else:
            response = {"error": "Invalid message type"}
        return response


def create_dispatcher():
    dispatcher = MessageDispatcher()
    handlers.bootstrap(dispatcher)
    return dispatcher
