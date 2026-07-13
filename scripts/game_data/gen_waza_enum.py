"""Regenerate the WazaID member lists from a Mappings.usmap — both the Python
enum in palworld_save_pal/game/enum.py and its duplicate TypeScript enum in
ui/src/lib/utils/pals.ts. The game inserts new EPalWazaID entries mid-enum
between versions, shifting the numeric values — both member lists must match
the game's enum exactly or numeric skill IDs in saves decode to the wrong
skill.

Usage: python scripts/game_data/gen_waza_enum.py <Mappings.usmap>
"""

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from usmap_enum import parse_enums  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
ENUM_PY = REPO_ROOT / "palworld_save_pal" / "game" / "enum.py"
PALS_TS = REPO_ROOT / "ui" / "src" / "lib" / "utils" / "pals.ts"


def member_names(waza: dict) -> list[tuple[str, int]]:
    out = []
    for name, value in sorted(waza.items(), key=lambda kv: kv[1]):
        if name == "None":
            name = "NONE"  # 'None' is not a valid identifier
        elif name == "EPalWazaID_MAX":
            name = "MAX"
        out.append((name, value))
    return out


def main() -> None:
    waza = parse_enums(sys.argv[1])["EPalWazaID"]
    members = member_names(waza)

    src = ENUM_PY.read_text(encoding="utf-8")
    py_block = "\n".join(f"    {n} = {v}" for n, v in members)
    pattern = re.compile(
        r"(class WazaID\(IntEnum\):\n(?:    \"\"\".*?\"\"\"\n)?\n?)"  # header
        r"(?:    \w+ = \d+\n)+",  # member block
    )
    new_src, n = pattern.subn(lambda m: m.group(1) + py_block + "\n", src, count=1)
    if n != 1:
        raise SystemExit("WazaID member block not found in enum.py")
    ENUM_PY.write_text(new_src, encoding="utf-8", newline="")
    print(f"enum.py WazaID regenerated: {len(waza)} entries (MAX = {waza['MAX']})")

    ts = PALS_TS.read_text(encoding="utf-8")
    ts_block = "\n".join(f"\t{n} = {v}," for n, v in members)
    ts_pattern = re.compile(r"(export enum WazaID \{\n)(?:\t\w+ = \d+,?\n)+(\})")
    new_ts, n = ts_pattern.subn(lambda m: m.group(1) + ts_block + "\n" + m.group(2), ts, count=1)
    if n != 1:
        raise SystemExit("WazaID enum block not found in pals.ts")
    PALS_TS.write_text(new_ts, encoding="utf-8", newline="")
    print(f"pals.ts WazaID regenerated: {len(waza)} entries")


if __name__ == "__main__":
    main()
