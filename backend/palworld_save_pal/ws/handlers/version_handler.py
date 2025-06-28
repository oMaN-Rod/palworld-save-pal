from palworld_save_pal.__version__ import __version__
from shared.models import MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def get_version_handler(_: dict, ws):
    """Handler for retrieving the current version of the application."""
    logger.debug("Getting current version")
    response = build_response(MessageType.GET_VERSION, __version__)
    await ws.send_json(response)
