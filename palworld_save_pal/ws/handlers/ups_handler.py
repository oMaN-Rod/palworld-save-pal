from typing import Optional
from uuid import UUID
from fastapi import WebSocket

from palworld_save_pal.game.pal import Pal
from palworld_save_pal.game.utils import format_character_key
from palworld_save_pal.state import get_app_state
from palworld_save_pal.db.ctx.ups import UPSService
from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.utils.uuid import are_equal_uuids
from palworld_save_pal.ws.messages import (
    GetUpsPalsMessage,
    GetUpsAllFilteredIdsMessage,
    AddUpsPalMessage,
    UpdateUpsPalMessage,
    DeleteUpsPalsMessage,
    CloneUpsPalMessage,
    CloneToUpsMessage,
    ExportUpsPalMessage,
    ImportToUpsMessage,
    GetUpsCollectionsMessage,
    CreateUpsCollectionMessage,
    UpdateUpsCollectionMessage,
    DeleteUpsCollectionMessage,
    GetUpsTagsMessage,
    CreateUpsTagMessage,
    UpdateUpsTagMessage,
    DeleteUpsTagMessage,
    GetUpsStatsMessage,
    NukeUpsPalsMessage,
    MessageType,
)
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


async def get_ups_all_filtered_ids_handler(
    message: GetUpsAllFilteredIdsMessage, ws: WebSocket
):
    try:
        data = message.data
        pal_ids = UPSService.get_all_filtered_pal_ids(
            search_query=data.search_query,
            character_id_filter=data.character_id_filter,
            collection_id=data.collection_id,
            tags=data.tags,
            element_types=data.element_types,
            pal_types=data.pal_types,
        )

        response_data = {
            "pal_ids": pal_ids,
            "total_count": len(pal_ids),
        }

        response = build_response(MessageType.GET_UPS_ALL_FILTERED_IDS, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error getting all filtered UPS pal IDs: %s", str(e))
        error_response = build_response(
            MessageType.ERROR,
            {"message": f"Failed to get filtered UPS pal IDs: {str(e)}"},
        )
        await ws.send_json(error_response)


async def get_ups_pals_handler(message: GetUpsPalsMessage, ws: WebSocket):
    try:
        data = message.data
        pals, total_count = UPSService.get_pals(
            offset=data.offset,
            limit=data.limit,
            search_query=data.search_query,
            character_id_filter=data.character_id_filter,
            collection_id=data.collection_id,
            tags=data.tags,
            element_types=data.element_types,
            pal_types=data.pal_types,
            sort_by=data.sort_by,
            sort_order=data.sort_order,
        )

        pal_list = []
        for pal in pals:
            pal_dict = {
                "id": pal.id,
                "instance_id": str(pal.instance_id),
                "character_id": pal.character_id,
                "character_key": format_character_key(pal.character_id),
                "nickname": pal.nickname,
                "level": pal.level,
                "pal_data": pal.pal_data,
                "source_save_file": pal.source_save_file,
                "source_player_uid": str(pal.source_player_uid)
                if pal.source_player_uid
                else None,
                "source_player_name": pal.source_player_name,
                "source_storage_type": pal.source_storage_type,
                "source_storage_slot": pal.source_storage_slot,
                "collection_id": pal.collection_id,
                "tags": pal.tags,
                "notes": pal.notes,
                "created_at": pal.created_at.isoformat(),
                "updated_at": pal.updated_at.isoformat(),
                "last_accessed_at": pal.last_accessed_at.isoformat()
                if pal.last_accessed_at
                else None,
                "transfer_count": pal.transfer_count,
                "clone_count": pal.clone_count,
            }
            pal_list.append(pal_dict)

        response_data = {
            "pals": pal_list,
            "total_count": total_count,
            "offset": data.offset,
            "limit": data.limit,
        }

        response = build_response(MessageType.GET_UPS_PALS, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error getting UPS pals: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to get UPS pals: {str(e)}"}
        )
        await ws.send_json(error_response)


async def add_ups_pal_handler(message: AddUpsPalMessage, ws: WebSocket):
    try:
        data = message.data
        ups_pal = UPSService.add_pal(
            pal_dto=data.pal_dto,
            source_save_file=data.source_save_file,
            source_player_uid=data.source_player_uid,
            source_player_name=data.source_player_name,
            source_storage_type=data.source_storage_type,
            source_storage_slot=data.source_storage_slot,
            collection_id=data.collection_id,
            tags=data.tags,
            notes=data.notes,
        )

        response = build_response(MessageType.ADD_UPS_PAL, ups_pal)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error adding UPS pal: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to add UPS pal: {str(e)}"}
        )
        await ws.send_json(error_response)


async def update_ups_pal_handler(message: UpdateUpsPalMessage, ws: WebSocket):
    try:
        data = message.data
        ups_pal = UPSService.update_pal(data.pal_id, data.updates)

        if not ups_pal:
            error_response = build_response(
                MessageType.ERROR,
                {"message": f"UPS Pal with ID {data.pal_id} not found"},
            )
            await ws.send_json(error_response)
            return

        response_data = {
            "pal": {
                "id": ups_pal.id,
                "instance_id": str(ups_pal.instance_id),
                "character_id": ups_pal.character_id,
                "nickname": ups_pal.nickname,
                "level": ups_pal.level,
                "collection_id": ups_pal.collection_id,
                "tags": ups_pal.tags,
                "notes": ups_pal.notes,
                "updated_at": ups_pal.updated_at.isoformat(),
            }
        }

        response = build_response(MessageType.UPDATE_UPS_PAL, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error updating UPS pal: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to update UPS pal: {str(e)}"}
        )
        await ws.send_json(error_response)


async def delete_ups_pals_handler(message: DeleteUpsPalsMessage, ws: WebSocket):
    try:
        data = message.data
        deleted_count = UPSService.delete_pals(data.pal_ids)

        response_data = {
            "deleted_count": deleted_count,
            "requested_count": len(data.pal_ids),
        }

        response = build_response(MessageType.DELETE_UPS_PALS, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error deleting UPS pals: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to delete UPS pals: {str(e)}"}
        )
        await ws.send_json(error_response)


async def clone_ups_pal_handler(message: CloneUpsPalMessage, ws: WebSocket):
    try:
        data = message.data
        cloned_pal = UPSService.clone_pal(data.pal_id)

        if not cloned_pal:
            error_response = build_response(
                MessageType.ERROR,
                {"message": f"UPS Pal with ID {data.pal_id} not found"},
            )
            await ws.send_json(error_response)
            return

        response_data = {
            "original_pal_id": data.pal_id,
            "cloned_pal": {
                "id": cloned_pal.id,
                "instance_id": str(cloned_pal.instance_id),
                "character_id": cloned_pal.character_id,
                "nickname": cloned_pal.nickname,
                "level": cloned_pal.level,
                "collection_id": cloned_pal.collection_id,
                "tags": cloned_pal.tags,
                "notes": cloned_pal.notes,
            },
        }

        response = build_response(MessageType.CLONE_UPS_PAL, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error cloning UPS pal: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to clone UPS pal: {str(e)}"}
        )
        await ws.send_json(error_response)


async def export_ups_pal_handler(message: ExportUpsPalMessage, ws: WebSocket):
    try:
        data = message.data
        app_state = get_app_state()

        if not app_state.save_file:
            error_response = build_response(
                MessageType.ERROR, {"message": "No save file loaded"}
            )
            await ws.send_json(error_response)
            return

        ups_pal = UPSService.get_pal_by_id(data.pal_id)
        if not ups_pal:
            error_response = build_response(
                MessageType.ERROR,
                {"message": f"UPS Pal with ID {data.pal_id} not found"},
            )
            await ws.send_json(error_response)
            return

        pal_dto = PalDTO.from_dict(other_pal=ups_pal.pal_data)

        save_file = app_state.save_file
        result = None
        player_name = None
        if data.destination_type == "pal_box":
            # Add to player's pal box with complete data preservation
            if not data.destination_player_uid:
                error_response = build_response(
                    MessageType.ERROR,
                    {"message": "Player UID required for pal box export"},
                )
                await ws.send_json(error_response)
                return
            player = save_file.get_players().get(data.destination_player_uid)
            if not player:
                error_response = build_response(
                    MessageType.ERROR, {"message": "Player not found"}
                )
                await ws.send_json(error_response)
                return
            result = save_file.add_player_pal_from_dto(
                player_id=data.destination_player_uid,
                pal_dto=pal_dto,
                container_id=player.pal_box_id,
            )
            player_name = player.nickname

        elif data.destination_type == "gps":
            # Add to global pal storage with complete data preservation
            result = save_file.add_gps_pal_from_dto(
                pal_dto=pal_dto,
                storage_slot=data.destination_slot,
            )

        elif data.destination_type == "dps":
            # Add to player's DPS with complete data preservation
            if not data.destination_player_uid:
                error_response = build_response(
                    MessageType.ERROR, {"message": "Player UID required for DPS export"}
                )
                await ws.send_json(error_response)
                return

            result = save_file.add_player_dps_pal_from_dto(
                player_id=data.destination_player_uid,
                pal_dto=pal_dto,
                storage_slot=data.destination_slot,
            )

        if result:
            destination_info = {
                "save_file_name": getattr(app_state.save_file, "name", "Unknown"),
                "player_uid": data.destination_player_uid,
                "player_name": player_name,
            }
            UPSService.export_pal_to_save(
                data.pal_id, data.destination_type, destination_info
            )

            # Send state refresh messages for immediate visibility
            if data.destination_type == "pal_box":
                # Send ADD_PAL message to update frontend state
                pal_data = result.model_dump()
                add_pal_data = {
                    "player_id": str(data.destination_player_uid),
                    "pal": pal_data,
                }
                add_pal_response = build_response(MessageType.ADD_PAL, add_pal_data)
                await ws.send_json(add_pal_response)

            elif data.destination_type == "dps":
                # Send ADD_DPS_PAL message to update frontend state
                slot_idx, pal = result
                add_dps_data = {
                    "player_id": str(data.destination_player_uid),
                    "pal": pal.model_dump(),
                    "index": slot_idx,
                }
                add_dps_response = build_response(MessageType.ADD_DPS_PAL, add_dps_data)
                await ws.send_json(add_dps_response)

            elif data.destination_type == "gps":
                slot_idx, pal = result
                add_gps_data = {
                    "pal": pal.model_dump(),
                    "index": slot_idx,
                }
                add_gps_response = build_response(MessageType.ADD_GPS_PAL, add_gps_data)
                await ws.send_json(add_gps_response)

            response_data = {
                "success": True,
                "destination_type": data.destination_type,
                "destination_player_uid": str(data.destination_player_uid)
                if data.destination_player_uid
                else None,
                "destination_slot": data.destination_slot,
            }
        else:
            response_data = {
                "success": False,
                "error": "Failed to export pal to destination",
            }

        response = build_response(MessageType.EXPORT_UPS_PAL, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error exporting UPS pal: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to export UPS pal: {str(e)}"}
        )
        await ws.send_json(error_response)


async def clone_to_ups_handler(message: CloneToUpsMessage, ws: WebSocket):
    try:
        data = message.data
        app_state = get_app_state()

        if not app_state.save_file:
            error_response = build_response(
                MessageType.ERROR, {"message": "No save file loaded"}
            )
            await ws.send_json(error_response)
            return

        save_file = app_state.save_file
        cloned_count = 0
        errors = []

        for pal_id in data.pal_ids:
            try:
                pal_dto = None
                source_info = {}

                match data.source_type:
                    case "pal_box":
                        if not data.source_player_uid:
                            errors.append(
                                f"Player UID required for pal box clone: {pal_id}"
                            )
                            continue

                        players = save_file.get_players()
                        player = players.get(UUID(data.source_player_uid))
                        if not player:
                            errors.append(f"Player not found {data.source_player_uid}")
                            continue

                        if not player.pals:
                            errors.append("Player has no pals")
                            continue

                        pal = player.pals.get(UUID(pal_id))
                        if not pal:
                            errors.append(
                                f"Pal not found in player's pal box: {pal_id}"
                            )
                            continue

                        pal_dto = PalDTO.from_dict(pal.model_dump())
                        source_info.update(
                            {
                                "save_file_name": getattr(save_file, "name", "Unknown"),
                                "player_uid": data.source_player_uid,
                                "player_name": player.nickname,
                                "storage_type": "pal_box",
                            }
                        )
                    case "gps":
                        gps_pals = save_file.get_gps()
                        if not gps_pals:
                            errors.append(f"GPS not available for: {pal_id}")
                            continue

                        pal = None
                        for slot_idx, gps_pal in gps_pals.items():
                            if are_equal_uuids(gps_pal.instance_id, pal_id):
                                pal = gps_pal
                                source_info["storage_slot"] = slot_idx
                                break

                        if not pal:
                            errors.append(f"Pal not found in GPS: {pal_id}")
                            continue

                        pal_dto = PalDTO.from_dict(pal.model_dump())
                        source_info.update(
                            {
                                "save_file_name": getattr(save_file, "name", "Unknown"),
                                "storage_type": "gps",
                            }
                        )
                    case "dps":
                        if not data.source_player_uid:
                            errors.append(
                                f"Player UID required for DPS clone: {pal_id}"
                            )
                            continue

                        player = save_file.get_players().get(
                            UUID(data.source_player_uid)
                        )
                        if not player or not player.dps:
                            errors.append(f"Player or DPS not found for: {pal_id}")
                            continue

                        pal: Optional[Pal] = None
                        for slot_idx, dps_pal in player.dps.items():
                            if are_equal_uuids(dps_pal.instance_id, pal_id):
                                pal = dps_pal
                                source_info["storage_slot"] = slot_idx
                                break

                        if not pal:
                            errors.append(f"Pal not found in DPS: {pal_id}")
                            continue

                        pal_dto = PalDTO.from_dict(pal.model_dump())
                        source_info.update(
                            {
                                "save_file_name": getattr(save_file, "name", "Unknown"),
                                "player_uid": data.source_player_uid,
                                "player_name": player.nickname,
                                "storage_type": "dps",
                            }
                        )

                if pal_dto:
                    player_uid = source_info.get("player_uid")
                    if player_uid and isinstance(player_uid, str):
                        player_uid = UUID(player_uid)

                    UPSService.add_pal(
                        pal_dto=pal_dto,
                        source_save_file=source_info.get("save_file_name"),
                        source_player_uid=player_uid,
                        source_player_name=source_info.get("player_name"),
                        source_storage_type=source_info.get("storage_type"),
                        source_storage_slot=source_info.get("storage_slot"),
                        collection_id=data.collection_id,
                        tags=data.tags,
                        notes=data.notes,
                    )
                    cloned_count += 1

            except Exception as e:
                logger.exception(f"Error cloning pal {pal_id} to UPS: {str(e)}")
                errors.append(f"Failed to clone {pal_id}: {str(e)}")

        response_data = {
            "success": cloned_count > 0,
            "cloned_count": cloned_count,
            "total_requested": len(data.pal_ids),
            "errors": errors,
        }

        response = build_response(MessageType.CLONE_TO_UPS, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error cloning to UPS: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to clone to UPS: {str(e)}"}
        )
        await ws.send_json(error_response)


async def import_to_ups_handler(message: ImportToUpsMessage, ws: WebSocket):
    try:
        data = message.data
        app_state = get_app_state()

        if not app_state.save_file:
            error_response = build_response(
                MessageType.ERROR, {"message": "No save file loaded"}
            )
            await ws.send_json(error_response)
            return

        save_file = app_state.save_file
        pal_dto = None
        source_info = {}
        match data.source_type:
            case "pal_box":
                if not data.source_pal_id or not data.source_player_uid:
                    error_response = build_response(
                        MessageType.ERROR,
                        {
                            "message": "Pal ID and Player UID required for pal box import"
                        },
                    )
                    await ws.send_json(error_response)
                    return

                player = save_file.get_players().get(data.source_player_uid)
                if not player or not player.pals:
                    error_response = build_response(
                        MessageType.ERROR, {"message": "Player or pals not found"}
                    )
                    await ws.send_json(error_response)
                    return

                pal = player.pals.get(data.source_pal_id)
                if not pal:
                    error_response = build_response(
                        MessageType.ERROR,
                        {"message": "Pal not found in player's pal box"},
                    )
                    await ws.send_json(error_response)
                    return

                pal_dto = PalDTO.from_dict(other_pal=pal.model_dump())
                source_info.update(
                    {
                        "save_file_name": getattr(save_file, "name", "Unknown"),
                        "player_uid": data.source_player_uid,
                        "player_name": player.nickname,
                        "storage_type": "pal_box",
                    }
                )
            case "gps":
                if data.source_slot is None:
                    error_response = build_response(
                        MessageType.ERROR,
                        {"message": "Slot index required for GPS import"},
                    )
                    await ws.send_json(error_response)
                    return

                gps_pals = save_file.get_gps()
                if not gps_pals or data.source_slot not in gps_pals:
                    error_response = build_response(
                        MessageType.ERROR, {"message": "Pal not found in GPS slot"}
                    )
                    await ws.send_json(error_response)
                    return

                pal = gps_pals[data.source_slot]
                pal_dto = PalDTO.from_dict(other_pal=pal.model_dump())
                source_info.update(
                    {
                        "save_file_name": getattr(save_file, "name", "Unknown"),
                        "storage_type": "gps",
                        "storage_slot": data.source_slot,
                    }
                )
            case "dps":
                if data.source_slot is None or not data.source_player_uid:
                    error_response = build_response(
                        MessageType.ERROR,
                        {
                            "message": "Slot index and Player UID required for DPS import"
                        },
                    )
                    await ws.send_json(error_response)
                    return

                player = save_file.get_players().get(data.source_player_uid)
                if not player or not player.dps:
                    error_response = build_response(
                        MessageType.ERROR, {"message": "Player or DPS not found"}
                    )
                    await ws.send_json(error_response)
                    return

                if data.source_slot not in player.dps:
                    error_response = build_response(
                        MessageType.ERROR, {"message": "Pal not found in DPS slot"}
                    )
                    await ws.send_json(error_response)
                    return

                pal: Pal = player.dps[data.source_slot]
                pal_dto = PalDTO.from_dict(other_pal=pal.model_dump())
                source_info.update(
                    {
                        "save_file_name": getattr(save_file, "name", "Unknown"),
                        "player_uid": data.source_player_uid,
                        "player_name": player.nickname,
                        "storage_type": "dps",
                        "storage_slot": data.source_slot,
                    }
                )

        if not pal_dto:
            error_response = build_response(
                MessageType.ERROR, {"message": "Failed to retrieve Pal data"}
            )
            await ws.send_json(error_response)
            return

        ups_pal = UPSService.add_pal(
            pal_dto=pal_dto,
            source_save_file=source_info.get("save_file_name"),
            source_player_uid=source_info.get("player_uid"),
            source_player_name=source_info.get("player_name"),
            source_storage_type=source_info.get("storage_type"),
            source_storage_slot=source_info.get("storage_slot"),
            collection_id=data.collection_id,
            tags=data.tags,
            notes=data.notes,
        )

        response_data = {
            "success": True,
            "pal": {
                "id": ups_pal.id,
                "instance_id": str(ups_pal.instance_id),
                "character_id": ups_pal.character_id,
                "nickname": ups_pal.nickname,
                "level": ups_pal.level,
                "collection_id": ups_pal.collection_id,
                "tags": ups_pal.tags,
                "notes": ups_pal.notes,
            },
        }

        response = build_response(MessageType.IMPORT_TO_UPS, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error importing to UPS: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to import to UPS: {str(e)}"}
        )
        await ws.send_json(error_response)


async def get_ups_collections_handler(_: GetUpsCollectionsMessage, ws: WebSocket):
    try:
        collections = UPSService.get_collections()

        collection_list = []
        for collection in collections:
            collection_dict = {
                "id": collection.id,
                "name": collection.name,
                "description": collection.description,
                "color": collection.color,
                "icon": collection.icon,
                "is_favorite": collection.is_favorite,
                "is_archived": collection.is_archived,
                "pal_count": collection.pal_count,
                "created_at": collection.created_at.isoformat(),
                "updated_at": collection.updated_at.isoformat(),
            }
            collection_list.append(collection_dict)

        response_data = {"collections": collection_list}
        response = build_response(MessageType.GET_UPS_COLLECTIONS, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error getting UPS collections: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to get UPS collections: {str(e)}"}
        )
        await ws.send_json(error_response)


async def create_ups_collection_handler(
    message: CreateUpsCollectionMessage, ws: WebSocket
):
    try:
        data = message.data
        collection = UPSService.create_collection(
            name=data.name,
            description=data.description,
            color=data.color,
        )

        response_data = {
            "collection": {
                "id": collection.id,
                "name": collection.name,
                "description": collection.description,
                "color": collection.color,
                "icon": collection.icon,
                "is_favorite": collection.is_favorite,
                "is_archived": collection.is_archived,
                "pal_count": collection.pal_count,
                "created_at": collection.created_at.isoformat(),
                "updated_at": collection.updated_at.isoformat(),
            }
        }

        response = build_response(MessageType.CREATE_UPS_COLLECTION, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error creating UPS collection: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to create UPS collection: {str(e)}"}
        )
        await ws.send_json(error_response)


async def update_ups_collection_handler(
    message: UpdateUpsCollectionMessage, ws: WebSocket
):
    try:
        data = message.data
        collection = UPSService.update_collection(data.collection_id, data.updates)

        if not collection:
            error_response = build_response(
                MessageType.ERROR,
                {"message": f"Collection with ID {data.collection_id} not found"},
            )
            await ws.send_json(error_response)
            return

        response_data = {
            "collection": {
                "id": collection.id,
                "name": collection.name,
                "description": collection.description,
                "color": collection.color,
                "icon": collection.icon,
                "is_favorite": collection.is_favorite,
                "is_archived": collection.is_archived,
                "pal_count": collection.pal_count,
                "created_at": collection.created_at.isoformat(),
                "updated_at": collection.updated_at.isoformat(),
            }
        }

        response = build_response(MessageType.UPDATE_UPS_COLLECTION, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error updating UPS collection: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to update UPS collection: {str(e)}"}
        )
        await ws.send_json(error_response)


async def delete_ups_collection_handler(
    message: DeleteUpsCollectionMessage, ws: WebSocket
):
    try:
        data = message.data
        success = UPSService.delete_collection(data.collection_id)

        response_data = {
            "success": success,
            "collection_id": data.collection_id,
        }

        response = build_response(MessageType.DELETE_UPS_COLLECTION, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error deleting UPS collection: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to delete UPS collection: {str(e)}"}
        )
        await ws.send_json(error_response)


async def get_ups_tags_handler(_: GetUpsTagsMessage, ws: WebSocket):
    try:
        tags = UPSService.get_available_tags()

        tag_list = []
        for tag in tags:
            tag_dict = {
                "id": tag.id,
                "name": tag.name,
                "description": tag.description,
                "color": tag.color,
                "usage_count": tag.usage_count,
                "created_at": tag.created_at.isoformat(),
                "updated_at": tag.updated_at.isoformat(),
            }
            tag_list.append(tag_dict)

        response_data = {"tags": tag_list}
        response = build_response(MessageType.GET_UPS_TAGS, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error getting UPS tags: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to get UPS tags: {str(e)}"}
        )
        await ws.send_json(error_response)


async def create_ups_tag_handler(message: CreateUpsTagMessage, ws: WebSocket):
    try:
        data = message.data
        tag = UPSService.create_or_update_tag(
            name=data.name,
            description=data.description,
            color=data.color,
        )

        response_data = {
            "tag": {
                "id": tag.id,
                "name": tag.name,
                "description": tag.description,
                "color": tag.color,
                "usage_count": tag.usage_count,
                "created_at": tag.created_at.isoformat(),
                "updated_at": tag.updated_at.isoformat(),
            }
        }

        response = build_response(MessageType.CREATE_UPS_TAG, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error creating UPS tag: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to create UPS tag: {str(e)}"}
        )
        await ws.send_json(error_response)


async def get_ups_stats_handler(_: GetUpsStatsMessage, ws: WebSocket):
    try:
        stats = UPSService.get_stats()

        response_data = {
            "stats": {
                "total_pals": stats.total_pals,
                "total_collections": stats.total_collections,
                "total_tags": stats.total_tags,
                "total_transfers": stats.total_transfers,
                "total_clones": stats.total_clones,
                "storage_size_mb": stats.storage_size_mb,
                "most_transferred_pal_id": stats.most_transferred_pal_id,
                "most_cloned_pal_id": stats.most_cloned_pal_id,
                "most_popular_character_id": stats.most_popular_character_id,
                "element_distribution": stats.element_distribution,
                "alpha_count": stats.alpha_count,
                "lucky_count": stats.lucky_count,
                "human_count": stats.human_count,
                "predator_count": stats.predator_count,
                "oilrig_count": stats.oilrig_count,
                "summon_count": stats.summon_count,
                "last_updated": stats.last_updated.isoformat(),
            }
        }

        response = build_response(MessageType.GET_UPS_STATS, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error getting UPS stats: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to get UPS stats: {str(e)}"}
        )
        await ws.send_json(error_response)


async def nuke_ups_pals_handler(message: NukeUpsPalsMessage, ws: WebSocket):
    """Nuke (delete) ALL pals from UPS storage."""
    try:
        # Get current count before deletion for response
        _, total_count = UPSService.get_pals(limit=1)

        if total_count == 0:
            response_data = {
                "success": True,
                "deleted_count": 0,
                "message": "UPS is already empty",
            }
        else:
            # Perform the nuke operation
            deleted_count = UPSService.nuke_all_pals()

            response_data = {
                "success": True,
                "deleted_count": deleted_count,
                "message": f"Successfully deleted {deleted_count} pals from UPS",
            }

        response = build_response(MessageType.NUKE_UPS_PALS, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error nuking UPS pals: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to nuke UPS pals: {str(e)}"}
        )
        await ws.send_json(error_response)


async def update_ups_tag_handler(message: UpdateUpsTagMessage, ws: WebSocket):
    try:
        data = message.data
        tag = UPSService.update_tag(data.tag_id, data.updates)

        if not tag:
            error_response = build_response(
                MessageType.ERROR,
                {"message": f"Tag with ID {data.tag_id} not found"},
            )
            await ws.send_json(error_response)
            return

        response_data = {
            "tag": {
                "id": tag.id,
                "name": tag.name,
                "description": tag.description,
                "color": tag.color,
                "usage_count": tag.usage_count,
                "created_at": tag.created_at.isoformat(),
                "updated_at": tag.updated_at.isoformat(),
            }
        }

        response = build_response(MessageType.UPDATE_UPS_TAG, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error updating UPS tag: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to update UPS tag: {str(e)}"}
        )
        await ws.send_json(error_response)


async def delete_ups_tag_handler(message: DeleteUpsTagMessage, ws: WebSocket):
    try:
        data = message.data
        success = UPSService.delete_tag(data.tag_id)

        response_data = {
            "success": success,
            "tag_id": data.tag_id,
        }

        response = build_response(MessageType.DELETE_UPS_TAG, response_data)
        await ws.send_json(response)

    except Exception as e:
        logger.exception("Error deleting UPS tag: %s", str(e))
        error_response = build_response(
            MessageType.ERROR, {"message": f"Failed to delete UPS tag: {str(e)}"}
        )
        await ws.send_json(error_response)
