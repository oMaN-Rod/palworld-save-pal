from fastapi import WebSocket

from palworld_save_pal.game.steam_id import (
    is_palworld_uid,
    parse_palworld_uid,
    parse_steam_input,
    player_uid_to_nosteam,
    steam_id_to_player_uid,
)
from palworld_save_pal.ws.messages import ConvertSteamIdMessage, MessageType
from palworld_save_pal.ws.utils import build_response


async def convert_steam_id_handler(message: ConvertSteamIdMessage, ws: WebSocket):
    raw = message.data.steam_input
    try:
        if is_palworld_uid(raw):
            palworld_uid = parse_palworld_uid(raw)
            nosteam_uid = player_uid_to_nosteam(palworld_uid)
            response = build_response(
                MessageType.CONVERT_STEAM_ID,
                {
                    "palworld_uid": str(palworld_uid).upper(),
                    "nosteam_uid": nosteam_uid.upper(),
                    "from_uid": True,
                },
            )
        else:
            steam_id = parse_steam_input(raw)
            palworld_uid = steam_id_to_player_uid(steam_id)
            nosteam_uid = player_uid_to_nosteam(palworld_uid)
            response = build_response(
                MessageType.CONVERT_STEAM_ID,
                {
                    "palworld_uid": str(palworld_uid).upper(),
                    "nosteam_uid": nosteam_uid.upper(),
                },
            )
    except ValueError as e:
        response = build_response(
            MessageType.CONVERT_STEAM_ID,
            {"error": str(e) if str(e) else "Invalid input. Enter a numeric Steam ID, profile URL, or Palworld UID."},
        )
    except OverflowError:
        response = build_response(
            MessageType.CONVERT_STEAM_ID,
            {"error": "Invalid Steam ID. Enter a numeric Steam ID or profile URL."},
        )
    await ws.send_json(response)
