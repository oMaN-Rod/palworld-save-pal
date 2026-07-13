# Game data regeneration pipeline

Regenerates the static game data under `data/json/` (plus l10n, the `WazaID`
enum, and UI icon assets) from a Palworld PAK dump, so the editor can be
updated for a new game version without hand-editing thousands of entries.

## 1. Produce a PAK dump

1. Get a mappings file for the target game version, e.g.
   [PalworldModding/UsefulFiles](https://github.com/PalworldModding/UsefulFiles)
   `Mappings.usmap`.
2. Export with [FModel](https://fmodel.app/) or
   [UnrealExporter](https://github.com/luk-gg/UnrealExporter) (UE 5.1, no AES
   key needed) as JSON/PNG:
   - `Pal/Content/Pal/DataTable/**` → json
   - `Pal/Content/L10N/**DataTable**` → json
   - `Pal/Content/Pal/Texture/PalIcon/**` → png
   - `Pal/Content/Others/InventoryItemIcon/**` → png

## 2. Regenerate data

```bash
# dry run, prints an added/changed/removed summary per file
python scripts/game_data/generate.py --dump-dir <dump>/Pal/Content --check

# write data/json/*.json and data/json/l10n/<lang>/*.json
python scripts/game_data/generate.py --dump-dir <dump>/Pal/Content

# regenerate the WazaID enum (numeric skill ids shift between versions!)
python scripts/game_data/gen_waza_enum.py <Mappings.usmap>

# fill missing icon webps referenced by pals.json/items.json
python scripts/game_data/convert_icons.py --dump-dir <dump> [--check]
```

Covered by `generate.py`: `pals`, `items`, `active_skills`, `passive_skills`,
`buildings`, `exp`, and l10n for those plus `technologies`, `lab_research`
(whose text keys are indirected through DataTable row fields), `elements` and
`work_suitability` (whose display names live in `DT_UI_Common_Text_Common`).

A dedicated-server pak works as a dump source for text data: it carries all
DataTables and the per-language `*_Common` L10N tables (recent content), but
not the full base text tables or any textures. Older entries then keep their
committed translations via the merge fallback below.

## Merge semantics

- Existing entries are refreshed from the dump (the game rebalances values
  between versions); hand-curated fields with no dump source (`disabled`,
  icon overrides, food `effect` for rows missing from `DT_StatusEffectFood`)
  are preserved. Existing key order is kept; new keys append alphabetically.
- pals.json: all monster rows except `BOSS_*` (resolved at runtime by prefix
  stripping) + all human rows. Passive effect types/targets the UI doesn't
  know (see `ui/src/lib/types/game.ts`) are recorded as `"None"`.
- items.json: existing keys + any new row named in the l10n item name table
  (unnamed rows are NPC/debug internals).
- l10n: RichText markup (`<itemName id=|X|/>`, style tags, including the
  game-typo variant that closes the id with `'` instead of `|`) is resolved
  against the same language's text tables; hard line breaks become spaces;
  `id-id` copies `id`. Unauthored game rows (`ko_Text`, `zh_Hans_Text`,
  `en Text`, bare `-`) are treated as missing, and the fallback chain is:
  own-language game text → previously committed translation → English game
  text → committed English translation → code name. A committed value that is
  itself a raw code name never shadows a better fallback. Known game-text
  typos are corrected via `HAND_FIXES` (keyed on the exact broken value, so
  each fix deactivates once the game ships corrected text).
- DataTable property-name casing differs between exporter/usmap versions
  ("PalId" vs "PalID"); rows are read case-insensitively.
