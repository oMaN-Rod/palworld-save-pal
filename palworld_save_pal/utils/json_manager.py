import json
import os
from typing import Any, Dict

from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class JsonManager:
    def __init__(self, file_path: str):
        self.file_path = file_path
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
        logger.info("Deleting key %s from %s", key, self.file_path)
        data = self.read()
        if key in data:
            del data[key]
            self.write(data)
