from typing import Dict, List
from uuid import UUID
from sqlmodel import select

from palworld_save_pal.editor.preset_profile import PresetProfile
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager
from palworld_save_pal.db.ctx.utils import get_db_session

logger = create_logger(__name__)


def get_all_presets() -> List[Dict]:
    with get_db_session() as session:
        statement = select(PresetProfile)
        results = session.exec(statement).all()

        presets_dict = {}
        for preset in results:
            preset_dict = preset.model_dump()
            preset_id = str(preset.id)
            presets_dict[preset_id] = preset_dict

        return presets_dict


def add_preset(preset_data: Dict) -> str:
    with get_db_session() as session:
        preset = PresetProfile(**preset_data)
        session.add(preset)
        session.flush()

        return str(preset.id)


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

            for _, preset_data in presets_data.items():
                if "id" in preset_data:
                    del preset_data["id"]

                preset = PresetProfile(**preset_data)
                session.add(preset)

            logger.info("Successfully populated presets table from JSON file.")
        else:
            logger.info(
                "Presets table already has data. Skipping population from JSON."
            )
