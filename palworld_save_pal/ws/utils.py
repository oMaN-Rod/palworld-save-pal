"""
This module contains utility functions and classes used in the API.

Functions:
- build_response: Build a response dictionary with the given message type,
                  status, data, and message.
- map_csv_row: Maps a CSV row to a dictionary representing the data for a model instance.
- build_error_response: Builds an error response based on the given exception.
- get_non_none_type: Returns the non-None type from a field type.
- build_mapping: Builds a mapping dictionary for a model class.
- import_handler: Handles the import of data from a CSV file.
- process_row: Processes a single row of data during the import process.
"""

import json
from typing import Any, Dict, Type
from uuid import UUID

from palworld_save_pal.ws.messages import MessageType
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


def build_response(message_type: MessageType, data: Any = None, message: str = None):
    """
    Build a response dictionary with the given message type, status, data, and message.

    Args:
        message_type (MessageType): The type of the message.
        status (MessageStatus): The status of the message.
        data (Any, optional): The data to include in the response. Defaults to None.
        message (str, optional): The message to include in the response. Defaults to None.

    Returns:
        dict: The response dictionary.
    """
    return {"type": message_type.value, "data": data, "message": message}
