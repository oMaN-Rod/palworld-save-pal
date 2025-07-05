from palworld_save_pal.utils.json_manager import JsonManager


PAL_DATA = JsonManager("data/json/pals.json").read()


def clean_character_id(character_id: str) -> str:
    character_key = ""
    typo_mapping = {
        "boss_police_old": "BOSS_Police_old",
        "police_handgun": "Police_Handgun",
    }
    if character_id.lower() in typo_mapping:
        character_id = typo_mapping[character_id.lower()]

    if character_id.lower().startswith("boss_") and character_id not in PAL_DATA:
        character_key = character_id[5:]
    elif character_id.lower().startswith("predator_"):
        character_key = character_id[9:]
    elif character_id.lower().endswith("_avatar"):
        character_key = character_id[:-7]
    else:
        character_key = character_id

    key_mapping = {
        "sheepball": "Sheepball",
        "lazycatfish": "LazyCatfish",
        "icedeer": "IceDeer",
        "blueplatypus": "BluePlatypus",
        "mopking": "MopKing",
    }

    if character_key.lower() in key_mapping:
        character_key = key_mapping[character_key.lower()]

    prefix_mapping = {
        "quest_farmer03_": "Quest_Farmer03_",
        "icenarwhal": "IceNarwhal",
    }

    for prefix, replacement in prefix_mapping.items():
        if character_key.lower().startswith(prefix):
            character_key = replacement + character_key[len(prefix) :]

    return character_id, character_key
