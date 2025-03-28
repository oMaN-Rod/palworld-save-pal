from pydantic import BaseModel, PrivateAttr, computed_field

from palworld_save_pal.db.models.settings_model import SettingsDTO
from palworld_save_pal.utils.file_manager import STEAM_ROOT
from palworld_save_pal.utils.logging_config import create_logger

from palworld_save_pal.db.ctx.settings import (
    get_settings,
    update_settings,
    update_save_dir,
)

logger = create_logger(__name__)


class Settings(BaseModel):
    _language: str = PrivateAttr(default="en")
    _save_dir: str = PrivateAttr(default=STEAM_ROOT)
    _clone_prefix: str = PrivateAttr(default="©️")
    _new_pal_prefix: str = PrivateAttr(default="🆕")
    _debug_mode: bool = PrivateAttr(default=False)
    _cheat_mode: bool = PrivateAttr(default=False)
    _is_busy: bool = PrivateAttr(default=True)

    def __init__(self, **data):
        super().__init__(**data)
        self._load_settings()

    @computed_field
    def language(self) -> str:
        return self._language

    @language.setter
    def language(self, value: str):
        self._language = value
        self._save()

    @computed_field
    def save_dir(self) -> str:
        return self._save_dir

    @save_dir.setter
    def save_dir(self, value: str):
        self._save_dir = value
        if not self._is_busy:
            update_save_dir(value)

    @computed_field
    def clone_prefix(self) -> str:
        return self._clone_prefix

    @clone_prefix.setter
    def clone_prefix(self, value: str):
        self._clone_prefix = value
        self._save()

    @computed_field
    def new_pal_prefix(self) -> str:
        return self._new_pal_prefix

    @new_pal_prefix.setter
    def new_pal_prefix(self, value: str):
        self._new_pal_prefix = value
        self._save()

    @computed_field
    def debug_mode(self) -> bool:
        return self._debug_mode

    @debug_mode.setter
    def debug_mode(self, value: bool):
        self._debug_mode = value
        self._save()

    @computed_field
    def cheat_mode(self) -> bool:
        return self._cheat_mode

    @cheat_mode.setter
    def cheat_mode(self, value: bool):
        self._cheat_mode = value
        self._save()

    def _save(self):
        if not self._is_busy:
            settings_dto = SettingsDTO(
                language=self._language,
                clone_prefix=self._clone_prefix,
                new_pal_prefix=self._new_pal_prefix,
                debug_mode=self._debug_mode,
                cheat_mode=self._cheat_mode,
            )
            update_settings(settings_dto)

    def _load_settings(self):
        try:
            db_settings = get_settings()

            self._language = db_settings.language
            self._save_dir = db_settings.save_dir
            self._clone_prefix = db_settings.clone_prefix
            self._new_pal_prefix = db_settings.new_pal_prefix
            self._debug_mode = db_settings.debug_mode
            self._cheat_mode = db_settings.cheat_mode

        except Exception as e:
            logger.warning(f"Error loading settings: {e}")

        self._is_busy = False

    def update_from(self, settings: SettingsDTO):
        self._is_busy = True

        self._language = settings.language
        self._clone_prefix = settings.clone_prefix
        self._new_pal_prefix = settings.new_pal_prefix
        self._debug_mode = settings.debug_mode
        self._cheat_mode = settings.cheat_mode

        update_settings(settings)
        self._is_busy = False
