"""Helpers for reading FModel/UnrealExporter DataTable JSON exports."""

import json
import re
from pathlib import Path


class CIRow(dict):
    """DataTable row with case-insensitive key access.

    Property-name casing varies between exporter/usmap versions
    ("PalId" vs "PalID", "Hp" vs "HP"), so exact-case lookups break on
    otherwise-identical dumps. Nested dicts are wrapped on access.
    """

    def __init__(self, d: dict):
        super().__init__(d)
        self._lower = {k.lower(): k for k in d}

    def __getitem__(self, key):
        if super().__contains__(key):
            value = super().__getitem__(key)
        else:
            value = super().__getitem__(self._lower[key.lower()])
        return CIRow(value) if isinstance(value, dict) and not isinstance(value, CIRow) else value

    def __contains__(self, key):
        return super().__contains__(key) or (
            isinstance(key, str) and key.lower() in self._lower
        )

    def get(self, key, default=None):
        try:
            return self[key]
        except KeyError:
            return default


def read_rows(dump_dir: Path, rel_path: str) -> dict:
    """Read the Rows dict from an exported DataTable JSON.

    Exports are a single-element list: [{"Type": "DataTable", ..., "Rows": {...}}].
    """
    path = Path(dump_dir) / rel_path
    with open(path, encoding="utf-8") as f:
        data = json.load(f)
    return {k: CIRow(v) if isinstance(v, dict) else v
            for k, v in data[0]["Rows"].items()}


def strip_enum(value: str) -> str:
    """'EPalElementType::Water' -> 'Water'. Non-str values pass through."""
    if isinstance(value, str) and "::" in value:
        return value.split("::")[-1]
    return value


# Unauthored rows in the game's text tables hold per-language placeholders
# like "ko_Text", "en Text", "zh-Hans_Text", "zh Hans Text" — treat them as
# missing (the lang/script separator appears as "-", "_", or a space
# between builds).
_PLACEHOLDER = re.compile(r"^[a-z]{2}(?:[-_ ][A-Za-z]{2,4})?[ _]Text$", re.IGNORECASE)


def localized_string(text_row: dict) -> str | None:
    """Extract the display string from an FText export row."""
    text_data = text_row.get("TextData") or {}
    value = text_data.get("LocalizedString")
    if value is None:
        value = text_row.get("SourceString")
    if isinstance(value, str):
        # Game text uses hard line breaks; PSP's l10n files never do.
        value = re.sub(r"\s*\r?\n\s*", " ", value).strip()
        # "-" is the game's other unauthored-row marker (used even in the
        # source-language tables for cut content).
        if _PLACEHOLDER.match(value) or value == "-":
            return None
    return value


def load_psp_json(path: Path) -> dict:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def save_psp_json(path: Path, data: dict) -> None:
    """Write JSON matching the repo's existing format: indent 2, UTF-8,
    unescaped non-ASCII, LF line endings, no trailing newline."""
    text = json.dumps(data, indent=2, ensure_ascii=False)
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(text)
