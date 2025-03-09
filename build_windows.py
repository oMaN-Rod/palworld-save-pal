#!/usr/bin/env python3
"""
build_windows.py - Script to automate the Windows build process
Creates both a standalone ZIP and MSI installer for the application
"""

from palworld_save_pal.__version__ import __version__
import os
import sys
import shutil
import zipfile
import subprocess
from pathlib import Path

APP_NAME = "Palworld Save Pal"
APP_VERSION = __version__


def cleanup_previous_builds():
    """Remove previous build artifacts"""
    print("Cleaning up previous builds...")
    
    # Directories to clean
    dirs_to_clean = ['build', 'dist']
    
    for directory in dirs_to_clean:
        if os.path.exists(directory):
            print(f"Removing {directory}/")
            shutil.rmtree(directory)

def run_build():
    """Run cx_Freeze to build the executable"""
    print("Building Windows executable...")
    
    # Run the build_exe command from cx_Freeze
    subprocess.check_call([sys.executable, "setup.py", "build_exe"])
    
    # Get the build directory (usually build/exe.win-*)
    build_dirs = list(Path("build").glob("exe.win*"))
    if not build_dirs:
        raise Exception("Build failed: No build directory found")
    
    build_dir = build_dirs[0]
    print(f"Build created in {build_dir}")
    
    return build_dir

def create_zip(build_dir):
    """Create a standalone ZIP file from the build directory"""
    print("Creating ZIP archive...")
    
    # Ensure dist directory exists
    os.makedirs("dist", exist_ok=True)
    
    # Create zip file
    zip_filename = f"dist/{APP_NAME}-{APP_VERSION}-windows.zip"
    with zipfile.ZipFile(zip_filename, 'w', zipfile.ZIP_DEFLATED) as zipf:
        # Walk through the build directory and add all files
        for root, _, files in os.walk(build_dir):
            for file in files:
                file_path = os.path.join(root, file)
                # Calculate the path within the zip file (relative to build_dir)
                arcname = os.path.relpath(file_path, build_dir)
                zipf.write(file_path, arcname)
    
    print(f"ZIP archive created: {zip_filename}")
    return zip_filename

def create_msi():
    """Create an MSI installer"""
    print("Creating MSI installer...")
    
    # Run the bdist_msi command from cx_Freeze
    subprocess.check_call([sys.executable, "setup.py", "bdist_msi"])
    
    # Find the created MSI file
    msi_files = list(Path("dist").glob("*.msi"))
    if not msi_files:
        raise Exception("MSI creation failed: No MSI file found")
    
    msi_file = msi_files[0]
    
    # Rename the MSI file to include the app name and version
    new_msi_name = f"dist/{APP_NAME}-{APP_VERSION}-windows.msi"
    if str(msi_file) != new_msi_name and os.path.basename(str(msi_file)) != os.path.basename(new_msi_name):
        os.rename(msi_file, new_msi_name)
        print(f"MSI installer created and renamed: {new_msi_name}")
    else:
        print(f"MSI installer created: {msi_file}")
        new_msi_name = str(msi_file)
    
    return new_msi_name

def main():
    """Main build process"""
    print(f"Starting Windows build process for {APP_NAME} v{APP_VERSION}")
    
    # Clean up previous builds
    cleanup_previous_builds()
    
    # Build the executable
    build_dir = run_build()
    
    # Create ZIP archive
    zip_file = create_zip(build_dir)
    
    # Create MSI installer
    msi_file = create_msi()
    
    print("\nBuild completed successfully!")
    print(f"ZIP: {zip_file}")
    print(f"MSI: {msi_file}")

if __name__ == "__main__":
    main()