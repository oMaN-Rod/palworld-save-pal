#!/bin/bash

# Build and Run Script for PALWorld Save Pal Desktop (Linux)
set -e

# Function to show help
show_help() {
    cat << EOF
Usage: ./build-desktop.sh [OPTIONS]

OPTIONS:
    -v, --version <version>    Set the version number (e.g., '1.0.0')
    -h, --help                 Show this help message

If no version is specified, the current version from __version__.py will be used.
EOF
    exit 0
}

# Function to update version in a file
update_version() {
    local file_path="$1"
    local new_version="$2"
    local pattern="$3"
    local replacement="$4"
    
    if [ -f "$file_path" ]; then
        sed -i.bak "$pattern" "$file_path"
        rm -f "$file_path.bak"
        echo "Updated version to $new_version in $file_path"
    else
        echo "Warning: File not found: $file_path" >&2
    fi
}

# Parse command line arguments
VERSION=""
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--version)
            VERSION="$2"
            shift 2
            ;;
        -h|--help)
            show_help
            ;;
        *)
            echo "Unknown option: $1" >&2
            echo "Use -h or --help for usage information." >&2
            exit 1
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Get or set version
if [ -n "$VERSION" ]; then
    # Validate version format (basic semver check)
    if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9\-\.]+)?(\+[a-zA-Z0-9\-\.]+)?$'; then
        echo "Error: Invalid version format. Please use semantic versioning (e.g., '1.0.0', '1.0.0-beta', '1.0.0+build.1')" >&2
        exit 1
    fi
    
    echo "Updating version to $VERSION..."
    
    # Update __version__.py
    update_version "./palworld_save_pal/__version__.py" "$VERSION" "s/__version__ = \"[^\"]*\"/__version__ = \"$VERSION\"/" ""
    
    # Update pyproject.toml
    update_version "./pyproject.toml" "$VERSION" "s/version = \"[^\"]*\"/version = \"$VERSION\"/" ""
else
    # Read current version
    VERSION=$(grep '__version__' ./palworld_save_pal/__version__.py | cut -d'"' -f2)
fi

echo "Building PALWorld Save Pal Desktop App version $VERSION"

DIST_DIR="./dist/psp-linux-$VERSION"
if [ -d "$DIST_DIR" ]; then
    echo "Removing existing distribution directory $DIST_DIR"
    rm -rf "$DIST_DIR"
fi
mkdir -p "$DIST_DIR"
echo "Created $DIST_DIR"

# Clean previous builds
[ -d "./build/" ] && echo "Removing existing build directory ./build/" && rm -rf ./build/
[ -d "./ui_build/" ] && echo "Removing existing ui_build directory ./ui_build/" && rm -rf ./ui_build/

# Set environment variables for the frontend
cat > ./ui/.env <<EOF
PUBLIC_WS_URL=127.0.0.1:5174/ws
PUBLIC_DESKTOP_MODE=true
EOF

cd ./ui

# Detect package manager
if command -v bun &>/dev/null; then
    PKG_MGR="bun"
elif command -v npm &>/dev/null; then
    PKG_MGR="npm"
elif command -v yarn &>/dev/null; then
    PKG_MGR="yarn"
else
    echo "No suitable package manager found. Please install Bun, npm, or Yarn."
    exit 1
fi

echo "Using $PKG_MGR as the package manager."

# Install dependencies
echo "Installing dependencies..."
$PKG_MGR install

# Build the frontend
echo "Building the frontend..."
$PKG_MGR run build

cd ..

echo "Building standalone executable with cx_Freeze..."
python3 setup.py build

echo "Copying files to distribution directory..."
cp -r ./build/exe.linux-*/* "$DIST_DIR" 2>/dev/null || cp -r ./build/exe.*/* "$DIST_DIR" 2>/dev/null

echo "Cleaning up..."
rm -rf ./ui_build/

# Create ZIP archive of the distribution files
ZIP_PATH="./dist/PalworldSavePal-$VERSION-linux-standalone.zip"
echo "Creating ZIP archive at $ZIP_PATH..."
if command -v zip &>/dev/null; then
    (cd dist && zip -r "$(basename "$ZIP_PATH")" "psp-linux-$VERSION")
else
    echo "zip not found, skipping archive creation."
fi

echo "Done building the desktop app for Linux."
