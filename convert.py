import argparse
import json
import os
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import Dict, Any, List, Set

from palworld_save_tools.paltypes import (
    DISABLED_PROPERTIES,
    PALWORLD_CUSTOM_PROPERTIES,
)

from palworld_save_pal.save_file.save_file import SaveFile
from palworld_save_pal.utils.logging_config import create_logger, setup_logging

save_file = SaveFile()
logger = create_logger(__name__)


class ValidationResults:
    def __init__(self):
        self.unknown_character_ids: Set[str] = set()
        self.unexpected_gender_values: Set[str] = set()
        self.unknown_active_skills: Set[str] = set()
        self.unknown_passive_skills: Set[str] = set()

    def log_results(self):
        if self.unknown_character_ids:
            logger.warning(
                f"Unknown character IDs: {', '.join(sorted(self.unknown_character_ids))}"
            )
        if self.unexpected_gender_values:
            logger.warning(
                f"Unexpected gender values: {', '.join(sorted(self.unexpected_gender_values))}"
            )
        if self.unknown_active_skills:
            logger.warning(
                f"Unknown active skills: {', '.join(sorted(self.unknown_active_skills))}"
            )
        if self.unknown_passive_skills:
            logger.warning(
                f"Unknown passive skills: {', '.join(sorted(self.unknown_passive_skills))}"
            )


def write_json_object(key, value, output_dir, force):
    output_file = os.path.join(output_dir, f"{key}.json")

    if os.path.exists(output_file) and not force:
        logger.warning("File %s already exists. Use --force to overwrite.", output_file)
        return False

    with open(output_file, "w", encoding="utf-8") as f:
        json.dump(value, f, indent=2)
    logger.info("Exported %s to %s", key, output_file)
    return True


def split_json_objects(json_file: str, output_dir: str, force: bool):
    logger.info("Splitting JSON objects")
    with open(json_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    world_save_data = (
        data.get("properties", {}).get("worldSaveData", {}).get("value", {})
    )

    if not world_save_data:
        logger.warning("No data found at properties > worldSaveData > value")
        return

    split_dir = os.path.join(output_dir, "split_objects")
    if not os.path.exists(split_dir):
        os.makedirs(split_dir)
        logger.info("Created split objects directory: %s", split_dir)

    with ThreadPoolExecutor() as executor:
        futures = []
        for key, value in world_save_data.items():
            future = executor.submit(write_json_object, key, value, split_dir, force)
            futures.append(future)

        for future in as_completed(futures):
            future.result()


def load_json_data(file_path: str) -> Dict[str, Any]:
    with open(file_path, "r", encoding="utf-8") as f:
        return json.load(f)


def validate_character_save_parameter_map(
    character_data: Dict[str, Any],
    pals: Dict[str, Any],
    active_skills: Dict[str, Any],
    passive_skills: Dict[str, Any],
) -> ValidationResults:
    logger.info("Validating CharacterSaveParameterMap")
    results = ValidationResults()

    for entry in character_data.get("value", []):
        save_parameter = (
            entry.get("value", {})
            .get("RawData", {})
            .get("value", {})
            .get("object", {})
            .get("SaveParameter", {})
            .get("value", {})
        )

        if save_parameter.get("IsPlayer", {}).get("value") == True:
            continue

        character_id = save_parameter.get("CharacterID", {}).get("value")
        character_id = character_id.replace("BOSS_", "")
        if character_id and character_id not in pals:
            results.unknown_character_ids.add(character_id)

        gender = save_parameter.get("Gender", {}).get("value", {}).get("value")
        if gender and gender not in ["EPalGenderType::Male", "EPalGenderType::Female"]:
            results.unexpected_gender_values.add(gender)

        for skill_list in ["EquipWaza", "MasteredWaza"]:
            skills = (
                save_parameter.get(skill_list, {}).get("value", {}).get("values", [])
            )
            for skill in skills:
                if skill not in active_skills:
                    results.unknown_active_skills.add(skill)

        passive_skills_list = (
            save_parameter.get("PassiveSkillList", {})
            .get("value", {})
            .get("values", [])
        )
        for passive_skill in passive_skills_list:
            if passive_skill not in passive_skills:
                results.unknown_passive_skills.add(passive_skill)

    return results


def main():
    parser = argparse.ArgumentParser(
        prog="palworld-save-tools",
        description="Converts Palworld save files to and from JSON, with optional splitting and validation",
    )
    parser.add_argument("filename")
    parser.add_argument(
        "--to-json",
        action="store_true",
        help="Override heuristics and convert SAV file to JSON",
    )
    parser.add_argument(
        "--from-json",
        action="store_true",
        help="Override heuristics and convert JSON file to SAV",
    )
    parser.add_argument(
        "--output",
        "-o",
        help="Output file (default: <filename>.json or <filename>.sav)",
    )
    parser.add_argument(
        "--force",
        "-f",
        action="store_true",
        help="Force overwriting output file if it already exists without prompting",
    )
    parser.add_argument(
        "--convert-nan-to-null",
        action="store_true",
        help="Convert NaN/Inf/-Inf floats to null when converting from SAV to JSON. This will lose information in the event Inf/-Inf is in the sav file (default: false)",
    )
    parser.add_argument(
        "--custom-properties",
        default=",".join(set(PALWORLD_CUSTOM_PROPERTIES.keys()) - DISABLED_PROPERTIES),
        type=lambda t: [s.strip() for s in t.split(",")],
        help="Comma-separated list of custom properties to decode, or 'all' for all known properties. This can be used to speed up processing by excluding properties that are not of interest. (default: all)",
    )
    parser.add_argument("--dev", action="store_true", help="Run in development mode")
    parser.add_argument("--minify-json", action="store_true", help="Minify JSON output")
    parser.add_argument(
        "--split", action="store_true", help="Split JSON objects after conversion"
    )
    parser.add_argument(
        "--validate", action="store_true", help="Validate data after splitting"
    )
    args = parser.parse_args()

    setup_logging(dev_mode=args.dev)

    if args.to_json and args.from_json:
        logger.error("Cannot specify both --to-json and --from-json")
        exit(1)

    if not os.path.exists(args.filename):
        logger.error("%s does not exist", args.filename)
        exit(1)
    if not os.path.isfile(args.filename):
        logger.error("%s is not a file", args.filename)
        exit(1)

    if args.to_json or args.filename.endswith(".sav"):
        logger.info("Converting %s to JSON", args.filename)
        save_file.name = args.filename
        if not args.output:
            output_path = args.filename + ".json"
        else:
            output_path = args.output
        logger.info("Loading GVAS")
        with open(args.filename, "rb") as f:
            data = f.read()
            save_file.load_level_sav(data)
        logger.info("Writing JSON to %s", output_path)
        save_file.to_json_file(
            output_path,
            minify=args.minify_json,
            allow_nan=(not args.convert_nan_to_null),
        )

        if args.split:
            logger.info("Splitting JSON objects")
            output_dir = os.path.dirname(output_path)
            split_json_objects(output_path, output_dir, args.force)

            if args.validate:
                logger.info("Validating data")
                data_dir = os.path.join(os.path.dirname(output_dir), "data", "json")
                character_data = load_json_data(
                    os.path.join(
                        output_dir, "split_objects", "CharacterSaveParameterMap.json"
                    )
                )
                pals = load_json_data(os.path.join(data_dir, "pals.json"))
                active_skills = load_json_data(
                    os.path.join(data_dir, "active_skills.json")
                )
                passive_skills = load_json_data(
                    os.path.join(data_dir, "passive_skills.json")
                )
                validation_results = validate_character_save_parameter_map(
                    character_data, pals, active_skills, passive_skills
                )
                validation_results.log_results()

    if args.from_json or args.filename.endswith(".json"):
        logger.info("Converting %s to SAV", args.filename)
        if not args.output:
            output_path = args.filename.replace(".json", "")
        else:
            output_path = args.output
        logger.info("Loading JSON")
        with open(args.filename, "rb") as f:
            data = f.read()
            save_file.load_json(data)
        logger.info("Writing SAV to %s", output_path)
        save_file.to_sav_file(output_path)


if __name__ == "__main__":
    main()
