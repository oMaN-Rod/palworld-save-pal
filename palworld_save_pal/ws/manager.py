import json
import traceback

from fastapi import WebSocket

from palworld_save_pal.ws.dispatcher import create_dispatcher
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.ws.messages import MessageType
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

dispatcher = create_dispatcher()


class ConnectionManager:
    def __init__(self):
        self.active_connections: list[WebSocket] = []
        self.websocket: WebSocket = None

    async def connect(self, websocket: WebSocket):
        await websocket.accept()
        self.active_connections.append(websocket)
        self.websocket = websocket

    def disconnect(self, websocket: WebSocket):
        self.active_connections.remove(websocket)
        self.websocket = None

    async def process_message(self, message: str, websocket: WebSocket):
        try:
            message_data = json.loads(message)
            logger.debug("Processing message type ==> %s", message_data["type"])
            await dispatcher.dispatch(message_data, websocket)
        except json.JSONDecodeError:
            logger.exception("Invalid JSON received: %s", message)
            exception = traceback.format_exc()
            response = build_response(
                MessageType.ERROR, f"Invalid JSON received:\n{exception}"
            )
            await websocket.send_json(response)
        except Exception as e:
            logger.exception("Error processing message: %s", str(e))
            exception = traceback.format_exc()
            data = {
                "message": str(e),
                "trace": exception,
            }
            response = build_response(MessageType.ERROR, data)
            await websocket.send_json(response)
