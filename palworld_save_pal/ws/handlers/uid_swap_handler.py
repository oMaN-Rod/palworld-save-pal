from fastapi import WebSocket

from palworld_save_pal.state import get_app_state
from palworld_save_pal.ws.messages import MessageType, SwapPlayerUidsMessage
from palworld_save_pal.ws.utils import build_response


async def swap_player_uids_handler(message: SwapPlayerUidsMessage, ws: WebSocket):
    app_state = get_app_state()
    save_file = app_state.save_file

    if not save_file:
        response = build_response(
            MessageType.SWAP_PLAYER_UIDS,
            {"error": "No save file loaded."},
        )
        await ws.send_json(response)
        return

    async def ws_callback(msg: str):
        progress = build_response(MessageType.PROGRESS_MESSAGE, msg)
        await ws.send_json(progress)

    result = await save_file.swap_player_uids(
        message.data.old_player_uid,
        message.data.new_player_uid,
        ws_callback=ws_callback,
    )

    if result.get("success"):
        app_state.player_summaries = save_file.get_player_summaries()
        app_state.guild_summaries = save_file.get_guild_summaries()

    response = build_response(MessageType.SWAP_PLAYER_UIDS, result)
    await ws.send_json(response)
