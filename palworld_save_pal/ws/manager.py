"""
This module contains the ConnectionManager class, which handles WebSocket connections
and message processing.
"""

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
    """
    ConnectionManager class handles WebSocket connections and message processing.

    Attributes:
        active_connections (list[WebSocket]): A list of active WebSocket connections.
        websocket (WebSocket): The current WebSocket connection.

    Methods:
        connect(websocket: WebSocket): Connects a WebSocket client.
        disconnect(websocket: WebSocket): Disconnects a WebSocket client.
        process_message(message: str, websocket: WebSocket): Processes a WebSocket message.
    """

    def __init__(self):
        self.active_connections: list[WebSocket] = []
        self.websocket: WebSocket = None

    async def connect(self, websocket: WebSocket):
        """
        Connects a WebSocket client.

        Args:
            websocket (WebSocket): The WebSocket client to connect.
        """
        await websocket.accept()
        self.active_connections.append(websocket)
        self.websocket = websocket

    def disconnect(self, websocket: WebSocket):
        """
        Disconnects a WebSocket client.

        Args:
            websocket (WebSocket): The WebSocket client to disconnect.
        """
        self.active_connections.remove(websocket)
        self.websocket = None

    async def process_message(self, message: str, websocket: WebSocket):
        """
        Processes a WebSocket message.

        Args:
            message (str): The message received from the WebSocket client.
            websocket (WebSocket): The WebSocket client that sent the message.
        """
        try:
            message_data = json.loads(message)
            logger.debug("Processing message type ==> %s", message_data["type"])
            await dispatcher.dispatch(message_data, websocket)
        except json.JSONDecodeError:
            logger.error("Invalid JSON")
            response = build_response(MessageType.ERROR, None, "Invalid JSON")
            await websocket.send_text(json.dumps(response))
        except Exception as e:
            logger.error("Unexpected error: %s", e)
            traceback.print_exc()
            response = build_response(MessageType.ERROR, None, str(e))
            await websocket.send_text(json.dumps(response))
