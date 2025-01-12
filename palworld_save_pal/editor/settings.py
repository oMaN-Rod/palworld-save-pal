import os
from pydantic import BaseModel, computed_field

from palworld_save_pal.utils.file_manager import STEAM_ROOT
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)

settings_json = JsonManager("data/json/settings.json")


class SettingsDTO(BaseModel):
    language: str
    save_dir: str
    clone_prefix: str
    new_pal_prefix: str


class Settings(BaseModel):
    _language: str = "en"
    _save_dir: str = STEAM_ROOT
    _clone_prefix: str = "[Clone]"
    _new_pal_prefix: str = "[New Pal]"
    _is_busy: bool = True

    def __init__(self):
        super().__init__()
        self._load_settings()

    @computed_field
    def language(self) -> str:
        return self._language

    @language.setter
    def language(self, value: str):
        self._language = value
        self.write()

    @computed_field
    def save_dir(self) -> str:
        return self._save_dir

    @save_dir.setter
    def save_dir(self, value: str):
        self._save_dir = value
        self.write()

    @computed_field
    def clone_prefix(self) -> str:
        return self._clone_prefix

    @clone_prefix.setter
    def clone_prefix(self, value: str):
        self._clone_prefix = value
        self.write()

    @computed_field
    def new_pal_prefix(self) -> str:
        return self._new_pal_prefix

    @new_pal_prefix.setter
    def new_pal_prefix(self, value: str):
        self._new_pal_prefix = value
        self.write()

    def write(self):
        if not self._is_busy:
            settings_json.write(self.model_dump())

    def _load_settings(self) -> "Settings":
        """Load settings from JSON file or return defaults"""
        try:
            saved_settings = settings_json.read()
            if saved_settings:
                for key, value in saved_settings.items():
                    setattr(self, key, value)
                self._is_busy = False
                return
        except Exception as e:
            self._is_busy = False
            logger.warning("Error loading settings: %s", e)

        # Return and save default settings if none exist
        self.write()

    def update_from(self, settings: SettingsDTO):
        self._is_busy = True
        for key, value in settings.model_dump().items():
            setattr(self, key, value)
        self._is_busy = False
        self.write()
