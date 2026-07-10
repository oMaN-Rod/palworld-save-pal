import os
from typing import TYPE_CHECKING, Any, Dict, Optional
from uuid import UUID

if TYPE_CHECKING:
    from palworld_save_pal.game.mixins._save_manager_protocol import (
        SaveManagerProtocol,
    )

    _Base = SaveManagerProtocol
else:
    _Base = object

from palworld_save_tools.gvas import GvasFile
from palworld_save_tools.palsav import compress_gvas_to_sav, decompress_sav_to_gvas
from palworld_save_tools.paltypes import (
    DISABLED_PROPERTIES,
    PALWORLD_CUSTOM_PROPERTIES,
    PALWORLD_TYPE_HINTS,
)

from palworld_save_pal.game.gvas_codec import CUSTOM_PROPERTIES
from palworld_save_pal.utils import json_io
from palworld_save_pal.utils.file_io import atomic_write
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.perf import gc_paused

logger = create_logger(__name__)


class SerializationMixin(_Base):
    def get_json(self, minify=False, allow_nan=True):
        logger.info("Converting %s to JSON", self.level_sav_path)
        with gc_paused():
            return json_io.dumps_str(
                self._gvas_file.dump(),
                indent=None if minify else 2,
                allow_nan=allow_nan,
            )

    def get_dict(self):
        logger.info("Converting %s to dict", self.level_sav_path)
        return self._gvas_file.dump()

    def load_json(self, data: bytes):
        logger.info("Loading %s as JSON", self.level_sav_path)
        with gc_paused():
            self._gvas_file = GvasFile.load(json_io.loads(data))
        return self

    def load_level_meta(self, data: bytes):
        logger.info("Loading %s as GVAS", self.level_sav_path)
        raw_gvas, _ = decompress_sav_to_gvas(data)
        custom_properties = {
            k: v
            for k, v in PALWORLD_CUSTOM_PROPERTIES.items()
            if k not in DISABLED_PROPERTIES
        }
        with gc_paused():
            gvas_file = GvasFile.read(
                raw_gvas, PALWORLD_TYPE_HINTS, custom_properties, allow_nan=True
            )
        self._level_meta_gvas_file = gvas_file
        return self._level_meta_gvas_file

    def load_level_sav(self, data: bytes):
        import time

        logger.info("Loading %s as GVAS", self.level_sav_path)
        start_time = time.perf_counter()
        raw_gvas, _ = decompress_sav_to_gvas(data)
        logger.info(f"Decompressed in {time.perf_counter() - start_time} seconds")
        gvas_start_time = time.perf_counter()
        with gc_paused():
            gvas_file = GvasFile.read(
                raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
            )
        logger.info(
            f"GvasFile read in {time.perf_counter() - gvas_start_time:.2f} seconds"
        )
        self._gvas_file = gvas_file
        self._get_file_size(data)
        return self

    def convert_sav_file_to_json(self, data: bytes, minify=True, allow_nan=True):
        logger.info("Converting to JSON")
        raw_gvas, _ = decompress_sav_to_gvas(data)
        with gc_paused():
            gvas_file = GvasFile.read(
                raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
            )
            return json_io.dumps_str(
                gvas_file.dump(),
                indent=None if minify else 2,
                allow_nan=allow_nan,
            )

    def convert_json_to_sav_file(self, data: bytes) -> bytes:
        logger.info("Converting JSON to SAV")
        with gc_paused():
            gvas_file = GvasFile.load(json_io.loads(data))
            raw_gvas = gvas_file.write(CUSTOM_PROPERTIES)
        sav_data = compress_gvas_to_sav(raw_gvas, 0x31)
        return sav_data

    def sav(self, gvas_file: GvasFile = None) -> bytes:
        logger.info("Converting %s to SAV", self.level_sav_path)
        target_gvas = gvas_file if gvas_file else self._gvas_file
        with gc_paused():
            raw = target_gvas.write(CUSTOM_PROPERTIES)
        return compress_gvas_to_sav(raw, 0x31)

    def player_savs(self) -> Dict[UUID, bytes]:
        logger.info("Converting player save files to SAV", len(self._player_gvas_files))
        with gc_paused():
            return {
                uid: compress_gvas_to_sav(
                    self._player_gvas_files[uid].sav.write(CUSTOM_PROPERTIES),
                    0x31,
                )
                for uid in self._player_gvas_files
            }

    def player_gvas_files(self) -> Dict[UUID, Dict[str, Optional[bytes]]]:
        logger.info(
            "Converting player save files to SAV: %s", len(self._player_gvas_files)
        )
        with gc_paused():
            return {
                uid: {
                    "sav": compress_gvas_to_sav(
                        files.sav.write(CUSTOM_PROPERTIES),
                        0x31,
                    ),
                    "dps": (
                        compress_gvas_to_sav(
                            files.dps.write(CUSTOM_PROPERTIES),
                            0x31,
                        )
                        if files.dps
                        else None
                    ),
                }
                for uid, files in self._player_gvas_files.items()
            }

    def gps_sav(self) -> Optional[bytes]:
        if not self._gps_gvas_file:
            return None
        logger.info("Converting GlobalPalStorage to SAV")
        with gc_paused():
            raw = self._gps_gvas_file.write(CUSTOM_PROPERTIES)
        return compress_gvas_to_sav(raw, 0x31)

    def to_json_file(
        self,
        output_path,
        minify=False,
        allow_nan=True,
    ):
        logger.info(
            "Converting %s to JSON, saving to %s", self.level_sav_path, output_path
        )
        with gc_paused():
            buf = json_io.dumps(
                self._gvas_file.dump(),
                indent=None if minify else 2,
                allow_nan=allow_nan,
            )
        atomic_write(output_path, buf)

    def level_meta_sav(self) -> Optional[bytes]:
        if not self._level_meta_gvas_file:
            return None
        logger.info("Converting LevelMeta to SAV bytes")
        with gc_paused():
            raw = self._level_meta_gvas_file.write(CUSTOM_PROPERTIES)
        return compress_gvas_to_sav(raw, 0x31)

    def to_level_sav_file(self, output_path):
        logger.info(
            "Converting %s to SAV, saving to %s", self.level_sav_path, output_path
        )
        with gc_paused():
            raw = self._gvas_file.write(CUSTOM_PROPERTIES)
        sav_file = compress_gvas_to_sav(raw, 0x31)
        atomic_write(output_path, sav_file)

    def to_level_meta_sav_file(self, output_path):
        if not self._level_meta_gvas_file:
            raise ValueError("No LevelMeta GvasFile has been loaded.")
        logger.info("Converting LevelMeta to SAV, saving to %s", output_path)
        with gc_paused():
            raw = self._level_meta_gvas_file.write(CUSTOM_PROPERTIES)
        sav_file = compress_gvas_to_sav(raw, 0x31)
        atomic_write(output_path, sav_file)

    def to_gps_save_file(self, output_path: str) -> None:
        if not self._gps_gvas_file:
            raise ValueError("No GPS GvasFile has been loaded.")
        logger.info("Converting GPS save file to SAV, saving to %s", output_path)
        with gc_paused():
            raw = self._gps_gvas_file.write(CUSTOM_PROPERTIES)
        sav_file = compress_gvas_to_sav(raw, 0x31)
        atomic_write(output_path, sav_file)

    def to_player_sav_files(self, output_path: str) -> None:
        logger.info("Converting player save files to SAV, saving to %s", output_path)
        with gc_paused():
            for uid, player_files in self._player_gvas_files.items():
                sav_file = compress_gvas_to_sav(
                    player_files.sav.write(CUSTOM_PROPERTIES),
                    0x31,
                )
                # GUID hex must be uppercase: Palworld reads player saves by
                # the uppercase filename, so a lowercase name orphans the record
                # on case-sensitive filesystems (Linux dedicated servers).
                uid = str(uid).replace("-", "").upper()
                atomic_write(os.path.join(output_path, f"{uid}.sav"), sav_file)
                if player_files.dps:
                    dps_sav_file = compress_gvas_to_sav(
                        player_files.dps.write(CUSTOM_PROPERTIES),
                        0x31,
                    )
                    atomic_write(
                        os.path.join(output_path, f"{uid}_dps.sav"), dps_sav_file
                    )
