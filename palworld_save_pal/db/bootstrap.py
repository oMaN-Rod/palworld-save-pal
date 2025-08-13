import os
import platform
import sys
from sqlmodel import SQLModel, create_engine

from palworld_save_pal.editor.preset_profile import PalPreset, PresetProfile
from palworld_save_pal.db.models.settings_model import SettingsModel
from palworld_save_pal.db.models.ups_models import (
    UPSPalModel,
    UPSCollectionModel, 
    UPSTagModel,
    UPSStatsModel,
    UPSTransferLogModel
)
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.db.migration import run_migrations

logger = create_logger(__name__)

SQLITE_URL = f"sqlite:///psp.db"
DB_PATH = "psp.db"

# If we're on Mac and frozen, make sure we use the correct path
if getattr(sys, "frozen", False) and platform.system() == "Darwin":
    # The application is frozen
    DB_PATH = os.path.join(os.path.dirname(sys.executable), "psp.db")
    SQLITE_URL = f"sqlite:///{DB_PATH}"

engine = create_engine(SQLITE_URL, echo=False)


def create_db_and_tables():
    if os.path.exists(DB_PATH):
        logger.info(
            f"Existing database found at {DB_PATH}, checking for schema updates"
        )
        run_migrations(DB_PATH)

    SQLModel.metadata.create_all(engine)
    logger.info("Database setup complete")
