import re
from uuid import UUID

from cityhash import CityHash64


def _u32(value: int) -> int:
    return int.from_bytes(
        (value & 0xFFFFFFFF).to_bytes(8, "little", signed=True),
        byteorder="little",
        signed=False,
    )


_HEX_UUID_RE = re.compile(r"^[0-9a-fA-F]{32}$")
_DASHED_UUID_RE = re.compile(
    r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
)


def parse_steam_input(raw: str) -> int:
    raw = raw.strip()
    if "steamcommunity.com/profiles/" in raw:
        raw = raw.split("steamcommunity.com/profiles/")[1].split("/")[0]
    elif "steamcommunity.com/id/" in raw:
        raise ValueError(
            "Vanity URLs (/id/) are not supported. "
            "Use the numeric Steam ID from the profile URL (/profiles/...) instead."
        )
    elif raw.startswith("steam_"):
        raw = raw[6:]
    return int(raw)


def is_palworld_uid(raw: str) -> bool:
    raw = raw.strip()
    return bool(_HEX_UUID_RE.match(raw) or _DASHED_UUID_RE.match(raw))


def parse_palworld_uid(raw: str) -> UUID:
    raw = raw.strip()
    if _HEX_UUID_RE.match(raw):
        raw = f"{raw[:8]}-{raw[8:12]}-{raw[12:16]}-{raw[16:20]}-{raw[20:]}"
    return UUID(raw)


def steam_id_to_player_uid(steam_id: int) -> UUID:
    hash_val = CityHash64(str(steam_id).encode("utf-16-le"))
    uid_int = _u32(_u32(hash_val) + (hash_val >> 32) * 23)
    return UUID(bytes=uid_int.to_bytes(4, byteorder="little") + b"\x00" * 12)


def player_uid_to_nosteam(player_uid: UUID) -> str:
    raw = int.from_bytes(player_uid.bytes[0:4], byteorder="little")
    a = _u32(_u32(raw << 8) ^ _u32(2654435769 - raw))
    b = _u32(a >> 13 ^ _u32(-(raw + a)))
    c = _u32(b >> 12 ^ _u32(raw - a - b))
    d = _u32(_u32(c << 16) ^ _u32(a - c - b))
    e = _u32(d >> 5 ^ b - d - c)
    f = _u32(e >> 3 ^ c - d - e)
    g = _u32(f << 10) ^ _u32(d - f - e)
    result = _u32(g >> 15 ^ e - g - f)
    return "%08X-0000-0000-0000-000000000000" % result
