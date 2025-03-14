from typing import Dict, List
from uuid import UUID
from sqlmodel import select
from sqlalchemy.orm import selectinload

from palworld_save_pal.editor.preset_profile import PresetProfile, PalPreset
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.db.ctx.utils import get_db_session

logger = create_logger(__name__)


def get_all_presets() -> Dict:
    with get_db_session() as session:
        statement = select(PresetProfile).options(
            selectinload(PresetProfile.pal_preset)
        )
        results = session.exec(statement).all()

        presets_dict = {}
        for preset in results:
            preset_dict = preset.model_dump()
            preset_id = str(preset.id)

            if preset.pal_preset:
                pal_preset_dict = preset.pal_preset.model_dump()
                preset_dict["pal_preset"] = pal_preset_dict

            presets_dict[preset_id] = preset_dict

        return presets_dict


def add_preset(preset_data: Dict) -> str:
    logger.debug(f"Adding preset with data: {preset_data}")
    with get_db_session() as session:
        try:
            if "pal_preset" in preset_data and preset_data["pal_preset"] is not None:
                pal_preset_data = preset_data.pop("pal_preset")
                pal_preset = PalPreset(**pal_preset_data)
                session.add(pal_preset)
                session.flush()

                preset = PresetProfile(**preset_data)
                preset.pal_preset_id = str(pal_preset.id)
                preset.pal_preset = pal_preset

                session.add(preset)
                session.commit()
            else:
                preset = PresetProfile(**preset_data)
                session.add(preset)
                session.commit()

            return str(preset.id)
        except Exception as e:
            logger.error(f"Error adding preset: {e}")
            session.rollback()
            raise


def update_preset_name(preset_id: UUID, new_name: str) -> bool:
    try:
        with get_db_session() as session:
            statement = select(PresetProfile).where(PresetProfile.id == preset_id)
            preset = session.exec(statement).one()
            preset.name = new_name
            return True
    except Exception as e:
        logger.error(f"Error updating preset name: {e}")
        return False


def delete_preset(preset_id: str) -> bool:
    logger.debug(f"Deleting preset with ID: {preset_id}")
    try:
        with get_db_session() as session:
            statement = select(PresetProfile).where(PresetProfile.id == preset_id)
            preset = session.exec(statement).one()
            session.delete(preset)
            return True
    except Exception as e:
        logger.error(f"Error deleting preset: {e}")
        return False


def populate_presets_from_json():
    with get_db_session() as session:
        statement = select(PresetProfile)
        existing_presets = session.exec(statement).all()

        if not existing_presets:
            logger.info("Presets table is empty. Populating from presets.json...")
            json_manager = JsonManager("data/json/presets.json")
            presets_data = json_manager.read()

            for preset_data in presets_data:
                preset = PresetProfile(**preset_data)
                session.add(preset)

            logger.info("Successfully populated presets table from JSON file.")
        else:
            logger.info(
                "Presets table already has data. Skipping population from JSON."
            )
