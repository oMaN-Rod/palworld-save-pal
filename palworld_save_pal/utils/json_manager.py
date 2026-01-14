import json
import os
import platform
import sys
from typing import Any, Dict

from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


def sanitize_string(value: str) -> str:
    if not value:
        return value
    try:
        # First try normal encoding - if it works, string is clean
        value.encode("utf-8")
        return value
    except UnicodeEncodeError:
        # String contains surrogates - remove them
        return value.encode("utf-8", errors="surrogatepass").decode(
            "utf-8", errors="replace"
        )


def find_data_file(filename):
    # If we're on Mac and frozen, make sure we use the correct path
    if getattr(sys, "frozen", False) and platform.system() == "Darwin":
        # The application is frozen
        datadir = os.path.dirname(sys.executable)
    else:
        # The application is not frozen
        datadir = ""
    return os.path.join(datadir, filename)


class JsonManager:
    def __init__(self, file_path: str):
        self.file_path = find_data_file(file_path)
        self.ensure_file_exists()

    def ensure_file_exists(self):
        if not os.path.exists(self.file_path):
            with open(self.file_path, "w", encoding="utf-8") as f:
                json.dump({}, f)

    def read(self) -> Dict[str, Any]:
        with open(self.file_path, "r", encoding="utf-8") as f:
            return json.load(f)

    def write(self, data: Dict[str, Any]):
        with open(self.file_path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2)

    def append(self, key: str, value: Any):
        data = self.read()
        data[key] = value
        self.write(data)

    def update_name(self, key: str, value: Any):
        data = self.read()
        entry = data.get(key, None)
        if entry is None:
            return
        entry["name"] = value
        self.write(data)

    def delete(self, key: str):
        data = self.read()
        if key in data:
            del data[key]
            self.write(data)
