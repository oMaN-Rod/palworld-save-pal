import os
import re


def rename_files():
    # Get all files in current directory
    files = os.listdir(".")

    # Pattern to match: T_*_icon_normal.png
    pattern = r"^T_(.+)_icon_normal\.png$"

    for filename in files:
        # Check if file matches our pattern
        match = re.match(pattern, filename)
        if match:
            # Get the middle part (group 1 from regex)
            base_name = match.group(1)

            # Create new filename:
            # 1. Convert to lowercase
            # 2. Add _menu.png suffix
            new_name = f"{base_name.lower()}_menu.png"

            try:
                os.rename(filename, new_name)
                print(f"Renamed '{filename}' to '{new_name}'")
            except OSError as e:
                print(f"Error renaming '{filename}': {e}")


if __name__ == "__main__":
    rename_files()
