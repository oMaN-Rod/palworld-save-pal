import argparse
import os

from palworld_save_tools.paltypes import (
    DISABLED_PROPERTIES,
    PALWORLD_CUSTOM_PROPERTIES,
)

from palworld_save_pal.save_file.save_file import SaveFile

save_file = SaveFile()


def main():
    parser = argparse.ArgumentParser(
        prog="palworld-save-tools",
        description="Converts Palworld save files to and from JSON",
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

    parser.add_argument("--minify-json", action="store_true", help="Minify JSON output")
    args = parser.parse_args()

    if args.to_json and args.from_json:
        print("Cannot specify both --to-json and --from-json")
        exit(1)

    if not os.path.exists(args.filename):
        print(f"{args.filename} does not exist")
        exit(1)
    if not os.path.isfile(args.filename):
        print(f"{args.filename} is not a file")
        exit(1)

    if args.to_json or args.filename.endswith(".sav"):
        save_file.name = args.filename
        if not args.output:
            output_path = args.filename + ".json"
        else:
            output_path = args.output
        with open(args.filename, "rb") as f:
            data = f.read()
            save_file.load_gvas(data)
        save_file.to_json(
            output_path,
            minify=args.minify_json,
            allow_nan=(not args.convert_nan_to_null),
        )

    if args.from_json or args.filename.endswith(".json"):
        if not args.output:
            output_path = args.filename.replace(".json", "")
        else:
            output_path = args.output
        with open(args.filename, "rb") as f:
            data = f.read()
            save_file.load_json(data)
        save_file.to_sav(output_path)


if __name__ == "__main__":
    main()
