import argparse
import json
import os
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import Dict, Any, Set
from deepdiff import DeepDiff


from palworld_save_pal.game.save_file import SaveFile
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
                "Unknown character IDs: %s",
                ", ".join(sorted(self.unknown_character_ids)),
            )
        if self.unexpected_gender_values:
            logger.warning(
                "Unexpected gender values: %s",
                ", ".join(sorted(self.unexpected_gender_values)),
            )
        if self.unknown_active_skills:
            logger.warning(
                "Unknown active skills: %s",
                ", ".join(sorted(self.unknown_active_skills)),
            )
        if self.unknown_passive_skills:
            logger.warning(
                "Unknown passive skills: %s",
                ", ".join(sorted(self.unknown_passive_skills)),
            )


class SaveComparison:
    TRACKED_OBJECTS = [
        "BaseCampSaveData",
        "CharacterContainerSaveData",
        "CharacterSaveParameterMap",
        "DynamicItemSaveData",
        "GroupSaveDataMap",
        "GuildExtraSaveDataMap",
        "InLockerCharacterInstanceIDArray",
        "ItemContainerSaveData",
    ]

    def __init__(self, original_data: Dict, modified_data: Dict, output_dir: str):
        self.original_data = self._extract_world_save_data(original_data)
        self.modified_data = self._extract_world_save_data(modified_data)
        self.differences = defaultdict(
            lambda: {"Added": [], "Deleted": [], "Updated": []}
        )
        self.output_dir = output_dir
        self.json_file = os.path.join(output_dir, "comparison_report.json")
        self.log_file = os.path.join(output_dir, "comparison_report.log")

    def log_differences(self):
        json_output = {}

        for obj_name, diff_types in self.differences.items():
            if any(diff_types.values()):
                json_output[obj_name] = {
                    "Added": diff_types["Added"],
                    "Deleted": diff_types["Deleted"],
                    "Updated": diff_types["Updated"],
                }

                # Log to console/log file for visibility
                header = f"\nDifferences in {obj_name}:"
                logger.info(header)

                for action, items in diff_types.items():
                    if items:
                        logger.info("  %s items:", action)
                        for item in items:
                            logger.info("    %s", item["path"])
                            if action == "Updated":
                                logger.info(
                                    "      Old: %s",
                                    json.dumps(item["old_value"], indent=2),
                                )
                                logger.info(
                                    "      New: %s",
                                    json.dumps(item["new_value"], indent=2),
                                )
                            else:
                                logger.info(
                                    "      Value: %s",
                                    json.dumps(item["value"], indent=2),
                                )

        # Write the JSON output
        with open(self.json_file, "w", encoding="utf-8") as f:
            json.dump(json_output, f, indent=2)

        logger.info("Comparison report saved to: %s", self.json_file)

    @staticmethod
    def _extract_world_save_data(data: Dict) -> Dict:
        return data.get("properties", {}).get("worldSaveData", {}).get("value", {})

    def compare(self):
        for obj_name in self.TRACKED_OBJECTS:
            original_obj = self.original_data.get(obj_name, {})
            modified_obj = self.modified_data.get(obj_name, {})

            if not original_obj and not modified_obj:
                continue

            diff = DeepDiff(original_obj, modified_obj, ignore_order=True)

            # Process additions
            self._process_additions(obj_name, diff)
            # Process deletions
            self._process_deletions(obj_name, diff)
            # Process changes
            self._process_changes(obj_name, diff)

    def _process_additions(self, obj_name: str, diff: DeepDiff):
        added_items = diff.get("dictionary_item_added", set())
        if added_items:
            for path in added_items:
                clean_path = path.replace("root", "")
                added_value = eval(f"self.modified_data['{obj_name}']{clean_path}")
                self.differences[obj_name]["Added"].append(
                    {"path": clean_path, "value": added_value}
                )

    def _process_deletions(self, obj_name: str, diff: DeepDiff):
        deleted_items = diff.get("dictionary_item_removed", set())
        if deleted_items:
            for path in deleted_items:
                clean_path = path.replace("root", "")
                deleted_value = eval(f"self.original_data['{obj_name}']{clean_path}")
                self.differences[obj_name]["Deleted"].append(
                    {"path": clean_path, "value": deleted_value}
                )

    def _process_changes(self, obj_name: str, diff: DeepDiff):
        changed_items = diff.get("values_changed", {})
        if changed_items:
            for path, change in changed_items.items():
                clean_path = path.replace("root", "")
                self.differences[obj_name]["Updated"].append(
                    {
                        "path": clean_path,
                        "old_value": change["old_value"],
                        "new_value": change["new_value"],
                    }
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


def split_json_objects(
    json_file: str, output_dir: str, force: bool, subfolder: str = ""
):
    logger.info("Splitting JSON objects")
    with open(json_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    world_save_data = (
        data.get("properties", {}).get("worldSaveData", {}).get("value", {})
    )

    if not world_save_data:
        logger.warning("No data found at properties > worldSaveData > value")
        return

    # Create subfolder within split_objects if specified
    split_dir = os.path.join(output_dir, "split_objects")
    if subfolder:
        split_dir = os.path.join(split_dir, subfolder)

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


def process_save_file(
    filename: str, output_path: str, minify: bool = False, convert_nan: bool = False
) -> Dict:
    save = SaveFile()
    save.level_sav_path = filename

    with open(filename, "rb") as f:
        data = f.read()
        save.load_level_sav(data)

    if output_path:
        save.to_json_file(
            output_path,
            minify=minify,
            allow_nan=(not convert_nan),
        )

    return json.loads(save.get_json(minify=minify, allow_nan=(not convert_nan)))


def main():
    parser = argparse.ArgumentParser(
        prog="palworld-save-tools",
        description="Converts Palworld save files to and from JSON, with optional splitting, validation, and comparison",
    )
    parser.add_argument("filename", help="Original save file")
    parser.add_argument(
        "--modified-save",
        help="Modified save file to compare against the original",
    )
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
        help="Convert NaN/Inf/-Inf floats to null when converting from SAV to JSON",
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

    # Handle comparison if modified save is provided
    if args.modified_save:
        if not os.path.exists(args.modified_save) or not os.path.isfile(
            args.modified_save
        ):
            logger.error(
                "Modified save file %s does not exist or is not a file",
                args.modified_save,
            )
            exit(1)

        logger.info("Comparing save files")

        # Create output directories for original and modified saves
        output_base = (
            os.path.dirname(args.output)
            if args.output
            else os.path.dirname(args.filename)
        )
        original_output = os.path.join(
            output_base, "original", os.path.basename(args.filename) + ".json"
        )
        modified_output = os.path.join(
            output_base, "modified", os.path.basename(args.modified_save) + ".json"
        )

        # Create directories if they don't exist
        os.makedirs(os.path.dirname(original_output), exist_ok=True)
        os.makedirs(os.path.dirname(modified_output), exist_ok=True)

        # Process both files
        original_data = process_save_file(
            args.filename, original_output, args.minify_json, args.convert_nan_to_null
        )
        modified_data = process_save_file(
            args.modified_save,
            modified_output,
            args.minify_json,
            args.convert_nan_to_null,
        )

        # Compare the files
        comparison = SaveComparison(original_data, modified_data, output_base)
        comparison.compare()
        comparison.log_differences()

        # Split files into separate directories
        split_json_objects(
            original_output, os.path.dirname(original_output), args.force, "original"
        )
        split_json_objects(
            modified_output, os.path.dirname(modified_output), args.force, "modified"
        )

        if args.validate:
            logger.info("Validating data")
            data_dir = os.path.join(os.path.dirname(original_output), "data", "json")
            character_data = load_json_data(
                os.path.join(
                    os.path.dirname(original_output),
                    "split_objects",
                    "CharacterSaveParameterMap.json",
                )
            )
            pals = load_json_data(os.path.join(data_dir, "pals.json"))
            active_skills = load_json_data(os.path.join(data_dir, "active_skills.json"))
            passive_skills = load_json_data(
                os.path.join(data_dir, "passive_skills.json")
            )
            validation_results = validate_character_save_parameter_map(
                character_data, pals, active_skills, passive_skills
            )
            validation_results.log_results()

    elif args.to_json or args.filename.endswith(".sav"):
        logger.info("Converting %s to JSON", args.filename)
        if not args.output:
            output_path = args.filename + ".json"
        else:
            output_path = args.output

        process_save_file(
            args.filename, output_path, args.minify_json, args.convert_nan_to_null
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

    elif args.from_json or args.filename.endswith(".json"):
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
        save_file.to_level_sav_file(output_path)


if __name__ == "__main__":
    main()
