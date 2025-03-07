from sqlmodel import select

from palworld_save_pal.db.ctx.utils import get_db_session
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.editor.settings import SettingsDTO
from palworld_save_pal.db.models.settings_model import SettingsModel

logger = create_logger(__name__)
settings_json = JsonManager("data/json/settings.json")


def get_settings() -> SettingsModel:
    with get_db_session() as session:
        statement = select(SettingsModel).where(SettingsModel.id == 1)
        settings = session.exec(statement).first()

        if not settings:
            logger.info("No settings found in database. Creating default settings.")
            try:
                saved_settings = settings_json.read()
                if saved_settings:
                    valid_fields = {
                        field for field in SettingsModel.__fields__ if field != "id"
                    }
                    filtered_settings = {
                        k: v for k, v in saved_settings.items() if k in valid_fields
                    }
                    settings = SettingsModel(**filtered_settings)
                else:
                    settings = SettingsModel()
            except Exception as e:
                logger.warning(f"Error loading settings from JSON: {e}")
                settings = SettingsModel()

            settings.id = 1
            session.add(settings)
            session.flush()

        session.expunge(settings)

        return settings


def update_settings(settings_dto: SettingsDTO) -> SettingsModel:
    with get_db_session() as session:
        statement = select(SettingsModel).where(SettingsModel.id == 1)
        settings = session.exec(statement).first()

        if not settings:
            settings = SettingsModel(id=1, **settings_dto.model_dump())
            session.add(settings)
        else:
            for key, value in settings_dto.model_dump().items():
                setattr(settings, key, value)

        return settings


def update_save_dir(save_dir: str) -> SettingsModel:
    with get_db_session() as session:
        statement = select(SettingsModel).where(SettingsModel.id == 1)
        settings = session.exec(statement).first()

        if not settings:
            settings = SettingsModel(id=1, save_dir=save_dir)
            session.add(settings)
        else:
            settings.save_dir = save_dir

        return settings
