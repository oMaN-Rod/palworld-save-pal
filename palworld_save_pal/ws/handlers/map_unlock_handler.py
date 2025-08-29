import shutil
from pathlib import Path
import traceback
from fastapi import WebSocket

from palworld_save_tools.gvas import GvasFile
from palworld_save_tools.palsav import compress_gvas_to_sav, decompress_sav_to_gvas
from palworld_save_tools.paltypes import PALWORLD_CUSTOM_PROPERTIES, PALWORLD_TYPE_HINTS

from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.ws.messages import MessageType, UnlockMapMessage
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)
app_state = get_app_state()


async def unlock_map_handler(message: UnlockMapMessage, ws: WebSocket):
    logger.debug("Unlocking map requested")

    async def ws_error(msg: str, trace: str = ""):
        response = build_response(
            MessageType.ERROR,
            {
                "message": msg,
                "trace": trace,
            },
        )
        await ws.send_json(response)

    try:
        file_path = message.data.path

        if not file_path:
            raise Exception("No file path provided")

        file_name = Path(file_path).name
        if file_name != "LocalData.sav":
            raise Exception("Please select the LocalData.sav file.")

        backup_path = file_path + ".backup"
        shutil.copy2(file_path, backup_path)

        with open(file_path, "rb") as f:
            local_data_bytes = f.read()

        raw_gvas, _ = decompress_sav_to_gvas(local_data_bytes)

        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, PALWORLD_CUSTOM_PROPERTIES, allow_nan=True
        )

        save_data = PalObjects.get_value(gvas_file.properties["SaveData"])
        if not save_data:
            raise Exception("SaveData not found in LocalData.sav")

        if "WorldMapMaskTextureV4" not in save_data:
            raise Exception("WorldMapMaskTextureV4 not found in SaveData")

        map_values = PalObjects.get_array_property(save_data["WorldMapMaskTextureV4"])

        if not map_values:
            raise Exception("Map values array not found")

        modified_count = 0
        map_values = list(map_values)

        for i in range(len(map_values)):
            if map_values[i] != 0:
                map_values[i] = 0
                modified_count += 1

        PalObjects.set_array_property(save_data["WorldMapMaskTextureV4"], map_values)

        sav_bytes = compress_gvas_to_sav(
            gvas_file.write(PALWORLD_CUSTOM_PROPERTIES), 0x31
        )

        # Write the modified file
        with open(file_path, "wb") as f:
            f.write(sav_bytes)

        response = build_response(
            MessageType.UNLOCK_MAP,
            {
                "success": True,
                "message": "Map unlocked successfully! Restart the game to see changes.",
            },
        )
        await ws.send_json(response)
    except Exception as e:
        await ws_error(f"Failed to unlock map: {str(e)}", traceback.format_exc())
