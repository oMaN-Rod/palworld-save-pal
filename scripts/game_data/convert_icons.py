"""Fill icon gaps in ui/src/lib/assets/img from a PAK texture dump.

Scans every `icon` reference in data/json/{pals,items}.json (plus
`icon_name` in technologies.json), diffs against the existing webp assets,
and converts the matching dumped PNGs. The UI globs the img directory at
build time, so dropping files in is all that's needed.

Usage:
    python scripts/game_data/convert_icons.py --dump-dir <datamine>/extracted \
        [--check]
"""

import argparse
import json
import re
import shutil
import sys
from pathlib import Path

from PIL import Image

# Variant markers used by character keys that share the base pal's art
# (same convention the UI's assetLoader cleansing uses at runtime).
_VARIANT_PREFIXES = ("predator_", "raid_", "summon_", "gym_", "boss_")
_VARIANT_SUFFIXES = ("_oilrig", "_max", "_otomo", "_quest", "_flower")


def base_icon_candidates(name: str) -> list[str]:
    """t_predator_garm_quest_icon_normal -> [t_garm_icon_normal, ...]"""
    m = re.fullmatch(r"t_(.+)_icon_normal", name)
    if not m:
        return []
    key = m.group(1)
    key = re.sub(r"_\d+$", "", key)
    for p in _VARIANT_PREFIXES:
        if key.startswith(p):
            key = key[len(p):]
    for s in _VARIANT_SUFFIXES:
        if key.endswith(s):
            key = key[: -len(s)]
    key = re.sub(r"_\d+$", "", key)
    return [f"t_{key}_icon_normal"] if key else []

REPO_ROOT = Path(__file__).resolve().parents[2]
DATA_DIR = REPO_ROOT / "data" / "json"
IMG_DIR = REPO_ROOT / "ui" / "src" / "lib" / "assets" / "img"


def referenced_icons() -> dict[str, str]:
    """icon base name (lowercase) -> where it's referenced (for reporting)"""
    refs = {}
    # technologies.json's icon_name is not consumed by the UI (no component
    # reads it), so only pal and item icons need webp assets.
    for fname, field in [("pals.json", "icon"), ("items.json", "icon")]:
        data = json.loads((DATA_DIR / fname).read_text(encoding="utf-8"))
        for key, entry in data.items():
            icon = entry.get(field)
            if icon:
                refs.setdefault(icon.lower(), f"{fname}:{key}")
    return refs


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dump-dir", required=True, type=Path,
                        help="Root of the PAK export (contains Pal/Content)")
    parser.add_argument("--check", action="store_true",
                        help="Report gaps without converting")
    args = parser.parse_args()

    refs = referenced_icons()
    existing = {p.stem.lower() for p in IMG_DIR.glob("*.webp")}
    missing = {name: src for name, src in refs.items() if name not in existing}
    print(f"{len(refs)} referenced icons, {len(existing)} webp present, "
          f"{len(missing)} missing")

    pngs = {p.stem.lower(): p for p in args.dump_dir.rglob("*.png")}
    converted, copied, unsourced = 0, 0, []
    for name, src in sorted(missing.items()):
        png = pngs.get(name)
        if png is not None:
            if not args.check:
                img = Image.open(png)
                img.save(IMG_DIR / f"{name}.webp", "WEBP", quality=90, method=6)
            converted += 1
            continue
        # Variant characters without their own texture reuse the base pal's
        # art (same as the existing gym_/quest webp copies in the repo).
        base = next((b for b in base_icon_candidates(name)
                     if b != name and (IMG_DIR / f"{b}.webp").exists()), None)
        if base:
            if not args.check:
                shutil.copyfile(IMG_DIR / f"{base}.webp", IMG_DIR / f"{name}.webp")
            copied += 1
            continue
        unsourced.append((name, src))
    print(f"{'would convert' if args.check else 'converted'}: {converted}, "
          f"{'would copy' if args.check else 'copied'} from base art: {copied}")
    if unsourced:
        print(f"no PNG source for {len(unsourced)}:")
        for name, src in unsourced:
            print(f"  {name}  (referenced by {src})")


if __name__ == "__main__":
    main()
