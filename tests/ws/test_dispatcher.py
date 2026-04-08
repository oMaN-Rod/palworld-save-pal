import pytest

from palworld_save_pal.ws.dispatcher import MessageDispatcher
from palworld_save_pal.ws.messages import BaseMessage


class TestMessageDispatcher:
    def test_register_handler(self):
        d = MessageDispatcher()
        handler = {
            "message_class": BaseMessage,
            "handler_func": lambda msg, ws: None,
        }
        d.register_handler("test_type", handler)
        assert "test_type" in d.handlers

    def test_register_multiple_handlers(self):
        d = MessageDispatcher()
        d.register_handler("type_a", {"message_class": BaseMessage, "handler_func": lambda m, w: None})
        d.register_handler("type_b", {"message_class": BaseMessage, "handler_func": lambda m, w: None})
        assert len(d.handlers) == 2

    @pytest.mark.asyncio
    async def test_dispatch_valid_type(self):
        d = MessageDispatcher()
        result_holder = {}

        async def handler_func(msg, ws):
            result_holder["called"] = True
            result_holder["data"] = msg.data
            return {"status": "ok"}

        d.register_handler("test_type", {
            "message_class": BaseMessage,
            "handler_func": handler_func,
        })

        response = await d.dispatch({"type": "test_type", "data": "hello"}, None)
        assert result_holder["called"] is True
        assert result_holder["data"] == "hello"
        assert response == {"status": "ok"}

    @pytest.mark.asyncio
    async def test_dispatch_invalid_type(self):
        d = MessageDispatcher()
        response = await d.dispatch({"type": "nonexistent"}, None)
        assert response == {"error": "Invalid message type"}

    @pytest.mark.asyncio
    async def test_dispatch_missing_type(self):
        d = MessageDispatcher()
        response = await d.dispatch({}, None)
        assert response == {"error": "Invalid message type"}
