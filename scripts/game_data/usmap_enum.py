"""Extract enum definitions from an unreal .usmap mappings file.

Only the name table and enum section are parsed (properties/structs are not
needed). Supports uncompressed usmap v0-v4; enum entry counts are u8 before
the LargeEnums revision (v3) and u16 from it onward.
"""

import struct


def _read_names(data: bytes, pos: int, version: int) -> tuple[list[str], int]:
    (count,) = struct.unpack_from("<I", data, pos)
    pos += 4
    names = []
    long_fname = version >= 2  # LongFName: u16 length prefix instead of u8
    for _ in range(count):
        if long_fname:
            (length,) = struct.unpack_from("<H", data, pos)
            pos += 2
        else:
            length = data[pos]
            pos += 1
        names.append(data[pos:pos + length].decode("utf-8", "replace"))
        pos += length
    return names, pos


def parse_enums(path: str) -> dict[str, list[str]]:
    with open(path, "rb") as f:
        data = f.read()

    (magic,) = struct.unpack_from("<H", data, 0)
    if magic != 0x30C4:
        raise ValueError(f"not a usmap file (magic {magic:#x})")
    version = data[2]

    # header: magic u16, version u8, [package versioning], method u8,
    # compressed u32, decompressed u32. Probe for the size pair instead of
    # hard-coding the versioning block layout.
    body = None
    for header_end in (8, 16):
        comp, decomp = struct.unpack_from("<II", data, header_end - 8)
        if comp == decomp == len(data) - header_end:
            body = data[header_end:]
            break
    if body is None:
        raise ValueError("compressed usmap or unknown header layout")

    names, pos = _read_names(body, 0, version)

    (enum_count,) = struct.unpack_from("<I", body, pos)
    pos += 4
    enums = {}
    for _ in range(enum_count):
        (name_idx,) = struct.unpack_from("<I", body, pos)
        pos += 4
        if version < 3:
            n_entries = body[pos]
            pos += 1
        else:
            (n_entries,) = struct.unpack_from("<H", body, pos)
            pos += 2
        entries = {}
        for _ in range(n_entries):
            if version >= 4:  # explicit i64 value per entry
                (value,) = struct.unpack_from("<q", body, pos)
                pos += 8
            else:
                value = len(entries)
            (idx,) = struct.unpack_from("<I", body, pos)
            pos += 4
            entries[names[idx]] = value
        enums[names[name_idx]] = entries
    return enums


if __name__ == "__main__":
    import sys

    enums = parse_enums(sys.argv[1])
    target = sys.argv[2] if len(sys.argv) > 2 else "EPalWazaID"
    print(f"{len(enums)} enums parsed")
    for entry, value in enums[target].items():
        print(value, entry)
