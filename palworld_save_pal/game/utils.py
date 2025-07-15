from palworld_save_pal.utils.json_manager import JsonManager


PAL_DATA = JsonManager("data/json/pals.json").read()


def clean_character_id(character_id: str) -> tuple[str, str]:
    character_id_lower = character_id.lower()

    if character_id_lower.startswith("boss_") and character_id not in PAL_DATA:
        character_key = character_id_lower[5:]
    elif character_id_lower.startswith("predator_"):
        character_key = character_id_lower[9:]
    elif character_id_lower.endswith("_avatar"):
        character_key = character_id_lower[:-7]
    else:
        character_key = character_id_lower

    return character_id, character_key
