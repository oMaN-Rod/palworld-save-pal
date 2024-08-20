import argparse
import json
import os
from concurrent.futures import ThreadPoolExecutor, as_completed

from palworld_save_tools.paltypes import (
    DISABLED_PROPERTIES,
    PALWORLD_CUSTOM_PROPERTIES,
)

from palworld_save_pal.save_file.save_file import SaveFile
from palworld_save_pal.utils.logging_config import create_logger, setup_logging

save_file = SaveFile()


def write_json_object(key, value, output_dir, force):
    logger = create_logger(__name__)
    output_file = os.path.join(output_dir, f"{key}.json")

    if os.path.exists(output_file) and not force:
        logger.warning("File %s already exists. Use --force to overwrite.", output_file)
        return False

    with open(output_file, "w", encoding="utf-8") as f:
        json.dump(value, f, indent=2)
    logger.info("Exported %s to %s", key, output_file)
    return True


def split_json_objects(json_file: str, output_dir: str, force: bool):
    logger = create_logger(__name__)

    # Read the JSON file
    with open(json_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    # Navigate to the specified path
    world_save_data = (
        data.get("properties", {}).get("worldSaveData", {}).get("value", {})
    )

    if not world_save_data:
        logger.warning("No data found at properties > worldSaveData > value")
        return

    # Create split objects directory
    split_dir = os.path.join(output_dir, "split_objects")
    if not os.path.exists(split_dir):
        os.makedirs(split_dir)
        logger.info("Created split objects directory: %s", split_dir)

    # Use ThreadPoolExecutor for concurrent writing
    with ThreadPoolExecutor() as executor:
        futures = []
        for key, value in world_save_data.items():
            future = executor.submit(write_json_object, key, value, split_dir, force)
            futures.append(future)

        # Wait for all tasks to complete
        for future in as_completed(futures):
            future.result()


def main():
    parser = argparse.ArgumentParser(
        prog="palworld-save-tools",
        description="Converts Palworld save files to and from JSON, with optional splitting",
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
    args = parser.parse_args()

    setup_logging(dev_mode=args.dev)

    logger = create_logger(__name__)

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
