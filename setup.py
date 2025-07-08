from cx_Freeze import setup, Executable
import sys
import os
from palworld_save_pal.__version__ import __version__

# Common build options for all platforms
build_exe_options = {
    "include_files": [
        ("ui_build", "ui"),
        ("data", "data"),
        ("debug.bat", "debug.bat")
        if sys.platform == "win32"
        else ("debug.sh", "debug.sh"),
    ],
    "packages": [
        "uvicorn",
        "fastapi",
        "webview",
        "palworld_save_tools",
        "websockets",
        "sqlalchemy.dialects.sqlite",
    ],
    "replace_paths": [("*", "")],
}

# Platform-specific configurations
if sys.platform == "win32":
    # Add Windows-specific files
    build_exe_options["include_files"].append(("debug.bat", "debug.bat"))
    
    # MSI installer options (Windows only)
    bdist_msi_options = {
        "add_to_path": True,
        "initial_target_dir": r"[ProgramFiles64Folder]\PalworldSavePal",
        "install_icon": "ui/static/favicon.ico",
        "upgrade_code": "{16ca64ed-033c-42d3-b0c7-5807be04d031}",
        "data": {
            "Directory": [
                ("ProgramMenuFolder", "TARGETDIR", "."),
                (
                    "ProgramMenuDir",
                    "ProgramMenuFolder",
                    "PalworldSavePal|Palworld Save Pal",
                ),
            ]
        },
    }
    
    # Windows-specific executable settings
    base = "Win32GUI"
    target_name = "PSP.exe"
    icon = "ui/static/favicon.ico"
    
elif sys.platform == "darwin":
    bdist_mac_options = {
        "bundle_name": "Palworld Save Pal",
        "iconfile": "ui/static/favicon.icns",
        "plist_items": [
            ("CFBundleShortVersionString", __version__),
            ("CFBundleIdentifier", "com.palworldsavepal"),
            ("NSHighResolutionCapable", True),
        ],
        "include_resources": [
            ("ui_build", "ui"),
            ("data", "data"),
        ],
    }

    # DMG specific options
    bdist_dmg_options = {
        "volume_label": f"PalworldSavePal-{__version__}-macOS",
        "format": "UDZO",
        "filesystem": "HFS+",
        "size": None,
        "background": "builtin-arrow",
        "show_status_bar": False,
        "show_tab_view": False,
        "show_path_bar": False,
        "show_sidebar": False,
        "sidebar_width": None,
        "show_icon_preview": False,
        "applications_shortcut": True,
    }

    base = None
    target_name = "psp"
    icon = "ui/static/favicon.icns"

elif sys.platform.startswith("linux"):
    # Linux-specific settings
    build_exe_options["include_files"].append(("linux_scripts/debug.sh", "debug.sh"))
    
    # For Ubuntu, we'll use both DEB and RPM options
    # DEB package options (Ubuntu/Debian)
    bdist_deb_options = {
        "depends": ["python3", "libgtk-3-0", "libwebkit2gtk-4.0-37"],
        "section": "games",
        "maintainer": "PalworldSavePal",
        "icon": "ui/static/favicon.png",  # Ensure you have a .png version for Linux
    }
    
    # RPM package options (for other Linux distros)
    bdist_rpm_options = {
        "requires": ["python3", "gtk3", "webkit2gtk3"],
        "group": "Applications/Games",
        "vendor": "PalworldSavePal",
        "icon": "ui/static/favicon.png",
    }
    
    # Linux-specific executable settings
    base = None
    target_name = "psp"
    icon = "ui/static/favicon.png"  # Use .png for Linux
    
    # Ensure executable permissions are set properly
    build_exe_options["bin_includes"] = []
    build_exe_options["bin_path_includes"] = ["/usr/bin"]
    
    # Additional Ubuntu 24.04-specific settings
    build_exe_options["includes"] = ["gi"]  # For GTK applications
    build_exe_options["include_files"].append(("/usr/lib/python3/dist-packages/gi", "lib/gi"))
    
    # Add desktop file for Ubuntu
    desktop_file = """[Desktop Entry]
Type=Application
Name=Palworld Save Pal
Comment=Palworld Save Manager
Exec=psp
Icon=/usr/share/icons/hicolor/256x256/apps/palworldsavepal.png
Categories=Game;Utility;
Terminal=false
"""
    with open("palworldsavepal.desktop", "w") as f:
        f.write(desktop_file)
    build_exe_options["include_files"].append(("palworldsavepal.desktop", "palworldsavepal.desktop"))
else:
    # Default for other platforms
    base = None
    target_name = "psp"
    icon = None
    
# Determine which installer options to include based on platform
installer_options = {}
if sys.platform == "win32":
    installer_options["bdist_msi"] = bdist_msi_options
elif sys.platform == "darwin":
    installer_options["bdist_mac"] = bdist_mac_options
    installer_options["bdist_dmg"] = bdist_dmg_options
elif sys.platform.startswith("linux"):
    # For Ubuntu, prioritize DEB packages but support RPM too
    try:
        import stdeb
        installer_options["bdist_deb"] = bdist_deb_options
        print("stdeb found - DEB package support enabled")
    except ImportError:
        print("stdeb not found - install python3-stdeb package for DEB support")
    
    try:
        # Check if RPM build tools are available
        installer_options["bdist_rpm"] = bdist_rpm_options
    except:
        print("RPM build tools not available")

# Combine build_exe options with installer options
all_options = {"build_exe": build_exe_options}
all_options.update(installer_options)

setup(
    name="PalworldSavePal",
    version=__version__,
    description="Palworld Save Pal",
    options=all_options,
    executables=[
        Executable(
            "desktop.py",
            base=base,
            icon=icon,
            target_name=target_name,
            shortcut_name="Palworld Save Pal",
            shortcut_dir="ProgramMenuDir" if sys.platform == "win32" else None,
        )
    ],
)