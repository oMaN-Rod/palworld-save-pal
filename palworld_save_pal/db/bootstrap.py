from sqlmodel import SQLModel, create_engine

from palworld_save_pal.editor.preset_profile import PalPreset, PresetProfile
from palworld_save_pal.db.models.settings_model import SettingsModel

SQLITE_URL = f"sqlite:///psp.db"

engine = create_engine(SQLITE_URL, echo=False)


def create_db_and_tables():
    SQLModel.metadata.create_all(engine)
