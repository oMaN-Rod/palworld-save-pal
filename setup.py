from cx_Freeze import setup, Executable
import sys
from palworld_save_pal.__version__ import __version__

# Import DMG settings from settings.py

# Build options
build_exe_options = {
    "include_files": [
        ("ui_build", "ui"),
        ("data", "data"),
        ("debug.bat", "debug.bat"),
    ],
    "packages": ["uvicorn", "fastapi", "webview", "palworld_save_tools", "websockets", "sqlalchemy.dialects.sqlite"],
    "replace_paths": [("*", "")],
}

# MSI installer options
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

# Mac specific options
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
    "volume_label": f"psp-mac-{__version__}",
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

base = "Win32GUI" if sys.platform == "win32" else None

setup(
    name="PalworldSavePal",
    version=__version__,
    description="Palworld Save Pal",
    options={
        "build_exe": build_exe_options,
        "bdist_msi": bdist_msi_options,
        "bdist_mac": bdist_mac_options,
        "bdist_dmg": bdist_dmg_options,
    },
    executables=[
        Executable(
            "desktop.py",
            base=base,
            icon="ui/static/favicon.ico",
            target_name="PSP.exe" if sys.platform == "win32" else "PSP",
            shortcut_name="Palworld Save Pal",
            shortcut_dir="ProgramMenuDir",
        )
    ],
)