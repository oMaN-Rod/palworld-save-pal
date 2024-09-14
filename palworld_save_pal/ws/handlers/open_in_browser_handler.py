import webbrowser
from fastapi import WebSocket
from palworld_save_pal.ws.messages import OpenInBrowserMessage, MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def open_in_browser_handler(message: OpenInBrowserMessage, ws: WebSocket):
    try:
        host = message.data
        port = host.split(":")[-1]
        host = host.replace(f":{port}", "")
        host = "localhost" if host == "127.0.0.1" else host
        webbrowser.open(f"http://{host}:{port}")
        response = build_response(
            MessageType.OPEN_IN_BROWSER, "Browser opened successfully"
        )
        await ws.send_json(response)
    except Exception as e:
        logger.error("Error opening browser: %s", str(e))
        response = build_response(MessageType.ERROR, f"Error opening browser: {str(e)}")
        await ws.send_json(response)
