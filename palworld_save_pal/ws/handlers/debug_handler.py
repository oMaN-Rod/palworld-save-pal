from fastapi import WebSocket
from palworld_save_pal.state import get_app_state
from palworld_save_pal.ws.messages import GetRawDataMessage, MessageType
from palworld_save_pal.ws.utils import build_response_custom
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def get_raw_data_handler(message: GetRawDataMessage, ws: WebSocket):
    logger.debug("Get Debug Handler")
    guild_id = message.data.guild_id
    player_id = message.data.player_id
    pal_id = message.data.pal_id
    base_id = message.data.base_id
    item_container_id = message.data.item_container_id
    character_container_id = message.data.character_container_id
    save_file = get_app_state().save_file
    data = {}
    if guild_id:
        guild = save_file.get_guild(guild_id)
        data = guild.save_data if guild else {}
    elif player_id:
        player = save_file.get_player(player_id)
        data = player.character_save if player else {}
    elif pal_id:
        pal = save_file.get_pal(pal_id)
        data = pal.character_save if pal else {}
    elif base_id:
        base = save_file.get_base(base_id)
        data = base.save_data if base else {}
    elif item_container_id:
        container = save_file.get_item_container(item_container_id)
        data = container if container else {}
    elif character_container_id:
        container = save_file.get_character_container(character_container_id)
        data = container if container else {}

    response = build_response_custom(MessageType.GET_RAW_DATA, data)
    await ws.send_json(response)
