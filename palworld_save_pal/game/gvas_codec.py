import base64
import re
from enum import Enum

from palworld_save_tools.archive import (
    FArchiveReader,
    FArchiveWriter,
)
from palworld_save_tools.paltypes import PALWORLD_CUSTOM_PROPERTIES


def skip_decode(reader: FArchiveReader, type_name: str, size: int, path: str):
    if type_name == "ArrayProperty":
        array_type = reader.fstring()
        value = {
            "skip_type": type_name,
            "array_type": array_type,
            "id": reader.optional_guid(),
            "value": reader.read(size),
        }
    elif type_name == "MapProperty":
        key_type = reader.fstring()
        value_type = reader.fstring()
        _id = reader.optional_guid()
        value = {
            "skip_type": type_name,
            "key_type": key_type,
            "value_type": value_type,
            "id": _id,
            "value": reader.read(size),
        }
    elif type_name == "StructProperty":
        value = {
            "skip_type": type_name,
            "struct_type": reader.fstring(),
            "struct_id": reader.guid(),
            "id": reader.optional_guid(),
            "value": reader.read(size),
        }
    else:
        raise ValueError(
            f"Expected ArrayProperty or MapProperty or StructProperty, got {type_name} in {path}"
        )
    return value


_LEGACY_HEX_RE = re.compile(r"^[0-9a-f]*$")


def _ensure_bytes(value):
    if isinstance(value, (bytes, bytearray)):
        return bytes(value)
    if isinstance(value, str):
        # Legacy palworld-save-tools emitted bytes as lowercase .hex();
        # current palworld-save-tools emits standard base64.
        if len(value) % 2 == 0 and _LEGACY_HEX_RE.match(value):
            return bytes.fromhex(value)
        return base64.b64decode(value, validate=True)
    return bytes(value)


def skip_encode(writer: FArchiveWriter, property_type: str, properties: dict) -> int:
    if "skip_type" not in properties:
        if (
            properties["custom_type"] in PALWORLD_CUSTOM_PROPERTIES
            and PALWORLD_CUSTOM_PROPERTIES[properties["custom_type"]] is not None
        ):
            return PALWORLD_CUSTOM_PROPERTIES[properties["custom_type"]][1](
                writer, property_type, properties
            )
        else:
            # Never be run to here
            return writer.property_inner(writer, property_type, properties)
    if property_type == "ArrayProperty":
        del properties["custom_type"]
        del properties["skip_type"]
        writer.fstring(properties["array_type"])
        writer.optional_guid(properties.get("id", None))
        data = _ensure_bytes(properties["value"])
        writer.write(data)
        return len(data)
    elif property_type == "MapProperty":
        del properties["custom_type"]
        del properties["skip_type"]
        writer.fstring(properties["key_type"])
        writer.fstring(properties["value_type"])
        writer.optional_guid(properties.get("id", None))
        data = _ensure_bytes(properties["value"])
        writer.write(data)
        return len(data)
    elif property_type == "StructProperty":
        del properties["custom_type"]
        del properties["skip_type"]
        writer.fstring(properties["struct_type"])
        writer.guid(properties["struct_id"])
        writer.optional_guid(properties.get("id", None))
        data = _ensure_bytes(properties["value"])
        writer.write(data)
        return len(data)
    else:
        raise ValueError(
            f"Expected ArrayProperty or MapProperty or StructProperty, got {property_type}"
        )


CUSTOM_PROPERTIES = {k: v for k, v in PALWORLD_CUSTOM_PROPERTIES.items()}
CUSTOM_PROPERTIES[".worldSaveData.FoliageGridSaveDataMap"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.MapObjectSpawnerInStageSaveData"] = (
    skip_decode,
    skip_encode,
)
CUSTOM_PROPERTIES[".worldSaveData.DungeonSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.EnemyCampSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.InvaderSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.DungeonPointMarkerSaveData"] = (
    skip_decode,
    skip_encode,
)
CUSTOM_PROPERTIES[".worldSaveData.GameTimeSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.OilrigSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.SupplySaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.BaseCampSaveData.Value.ModuleMap"] = (
    skip_decode,
    skip_encode,
)


class SaveType(int, Enum):
    STEAM = 0
    GAMEPASS = 1