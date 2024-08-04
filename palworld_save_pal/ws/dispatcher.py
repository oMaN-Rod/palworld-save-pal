"""
This module contains the implementation of a message dispatcher for a WebSocket server.
"""

import palworld_save_pal.ws.handlers.bootstrap as handlers


class MessageDispatcher:
    """
    A class that handles message dispatching for a WebSocket server.

    Attributes:
        handlers (dict): A dictionary that maps message types to their corresponding handlers.
    """

    def __init__(self):
        self.handlers = {}

    def register_handler(self, message_type, handler):
        """
        Registers a handler for a specific message type.

        Args:
            message_type (str): The type of the message.
            handler (dict): A dictionary containing the message class and handler function.

        Example:
            >>> dispatcher.register_handler(MessageType.GET_CUSTOMERS.value,
                                            {'message_class': GetCustomersMessage,
                                            'handler_func': customer_handler.get_customers_handler})
        """
        self.handlers[message_type] = handler

    async def dispatch(self, message_data, websocket):
        """
        Dispatches a message to its corresponding handler.

        Args:
            message_data (dict): The data of the message.
            websocket: The WebSocket connection.

        Returns:
            dict: The response from the handler.

        Example:
            >>> response = await dispatcher.dispatch(message_data, websocket)
        """
        message_type = message_data.get("type")
        handler = self.handlers.get(message_type)
        if handler:
            message = handler["message_class"](**message_data)
            response = await handler["handler_func"](message, websocket)
        else:
            response = {"error": "Invalid message type"}
        return response


def create_dispatcher():
    """
    Creates a new instance of the MessageDispatcher class and performs bootstrap.

    Returns:
        MessageDispatcher: The created message dispatcher.

    Example:
        >>> dispatcher = create_dispatcher()
    """
    dispatcher = MessageDispatcher()
    handlers.bootstrap(dispatcher)
    return dispatcher
