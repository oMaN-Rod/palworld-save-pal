from typing import Any, Dict
from fastapi import WebSocket
from palworld_save_pal.state import get_app_state
from palworld_save_pal.ws.messages import (
    AddGpsPalMessage,
    CloneGpsPalMessage,
    CloneGpsPalToPlayerMessage,
    DeleteGpsPalsMessage,
    RequestGpsMessage,
    MessageType,
)
from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.utils.uuid import are_equal_uuids
from uuid import UUID
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def request_gps_handler(message: RequestGpsMessage, ws: WebSocket):
    app_state = get_app_state()

    if not app_state.save_file:
        logger.error("No save file loaded")
        await ws.send_json(
            build_response(
                MessageType.GET_GPS_RESPONSE,
                {"error": "No save file loaded"},
            )
        )
        return

    async def progress_callback(msg: str):
        await ws.send_json(build_response(MessageType.PROGRESS_MESSAGE, msg))

    if app_state.gps_loaded and app_state.gps:
        logger.info("GPS already loaded, returning cached data")
        await ws.send_json(build_response(MessageType.GET_GPS_RESPONSE, app_state.gps))
        return

    gps = await app_state.load_gps_on_demand(progress_callback)

    if gps is not None:
        await ws.send_json(build_response(MessageType.GET_GPS_RESPONSE, gps))
        logger.info(f"Sent GPS data with {len(gps)} pals")
    else:
        await ws.send_json(
            build_response(
                MessageType.GET_GPS_RESPONSE,
                {"available": False, "message": "No GPS file available for this save"},
            )
        )


async def add_gps_pal_handler(message: AddGpsPalMessage, ws: WebSocket):
    character_id = message.data.character_id
    nickname = message.data.nickname
    storage_slot = message.data.storage_slot

    app_state = get_app_state()
    save_file = app_state.save_file

    if not app_state.save_file:
        return

    res = save_file.add_gps_pal(character_id, nickname, storage_slot)
    if not res:
        # Failed to add pal, possibly due to no available slots
        response = build_response(
            MessageType.ADD_GPS_PAL,
            {"error": "Failed to add pal. No available slots or invalid data."},
        )
        await ws.send_json(response)
        return
    new_pal, slot_idx = res
    data: Dict[str, Any] = {"pal": new_pal, "index": slot_idx}
    response = build_response(MessageType.ADD_GPS_PAL, data)
    await ws.send_json(response)


async def clone_gps_pal_handler(message: CloneGpsPalMessage, ws: WebSocket):
    pal = message.data.pal

    app_state = get_app_state()
    save_file = app_state.save_file

    if not save_file:
        return

    res = save_file.clone_gps_pal(pal)
    if not res:
        # Failed to clone, possibly due to no available slots
        response = build_response(
            MessageType.ADD_GPS_PAL,
            {"error": "Failed to clone pal. No available slots."},
        )
        await ws.send_json(response)
        return
    slot_idx, new_pal = res
    data: Dict[str, Any] = {"pal": new_pal, "index": slot_idx}
    response = build_response(MessageType.ADD_GPS_PAL, data)
    await ws.send_json(response)


async def delete_gps_pals_handler(message: DeleteGpsPalsMessage, _: WebSocket):
    pal_indexes = message.data.pal_indexes
    logger.debug(f"Deleting {len(pal_indexes)} GPS pals")
    app_state = get_app_state()
    save_file = app_state.save_file
    if not save_file:
        logger.warning("No save file loaded, cannot delete GPS pals")
        return
    save_file.delete_gps_pals(pal_indexes)


async def clone_gps_pal_to_player_handler(
    message: CloneGpsPalToPlayerMessage, ws: WebSocket
):
    data = message.data
    app_state = get_app_state()
    save_file = app_state.save_file

    if not save_file:
        await ws.send_json(
            build_response(MessageType.ERROR, {"message": "No save file loaded"})
        )
        return

    if data.destination_type not in ("pal_box", "dps"):
        await ws.send_json(
            build_response(
                MessageType.ERROR,
                {"message": f"Invalid destination type: {data.destination_type}"},
            )
        )
        return

    player = save_file.get_players().get(UUID(data.destination_player_uid))
    if not player:
        await ws.send_json(
            build_response(MessageType.ERROR, {"message": "Player not found"})
        )
        return

    gps_pals = save_file.get_gps()
    if not gps_pals:
        await ws.send_json(
            build_response(MessageType.ERROR, {"message": "GPS not available"})
        )
        return

    cloned_count = 0
    errors = []

    for pal_id in data.pal_ids:
        source_pal = None
        for _, gps_pal in gps_pals.items():
            if are_equal_uuids(gps_pal.instance_id, pal_id):
                source_pal = gps_pal
                break

        if not source_pal:
            errors.append(f"Pal not found in GPS: {pal_id}")
            continue

        pal_dto = PalDTO.from_dict(source_pal.model_dump())

        if data.destination_type == "pal_box":
            new_pal = save_file.add_player_pal_from_dto(
                player_id=UUID(data.destination_player_uid),
                pal_dto=pal_dto,
                container_id=player.pal_box_id,
            )
            if not new_pal:
                errors.append(f"Failed to add pal to pal box: {pal_id}")
                continue
            await ws.send_json(
                build_response(
                    MessageType.ADD_PAL,
                    {
                        "player_id": str(player.uid),
                        "pal": new_pal.model_dump(),
                    },
                )
            )
            cloned_count += 1
        else:  # dps
            result = save_file.add_player_dps_pal_from_dto(
                player_id=UUID(data.destination_player_uid),
                pal_dto=pal_dto,
            )
            if not result:
                errors.append(f"Failed to add pal to DPS: {pal_id}")
                continue
            slot_idx, new_pal = result
            await ws.send_json(
                build_response(
                    MessageType.ADD_DPS_PAL,
                    {
                        "player_id": str(player.uid),
                        "pal": new_pal.model_dump(),
                        "index": slot_idx,
                    },
                )
            )
            cloned_count += 1

    await ws.send_json(
        build_response(
            MessageType.CLONE_GPS_PAL_TO_PLAYER,
            {
                "success": cloned_count > 0,
                "cloned_count": cloned_count,
                "errors": errors,
            },
        )
    )
