from cx_Freeze import setup, Executable
import sys
from palworld_save_pal.__version__ import __version__

build_exe_options = {
    "include_files": [
        ("ui_build", "ui"),
        ("data", "data"),
        ("debug.bat", "debug.bat"),
    ],
    "packages": ["uvicorn", "fastapi", "webview", "palworld_save_tools", "websockets"],
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

base = "Win32GUI" if sys.platform == "win32" else None

setup(
    name="PalworldSavePal",
    version=__version__,
    description="Palworld Save Pal",
    options={"build_exe": build_exe_options, "bdist_msi": bdist_msi_options},
    executables=[
        Executable(
            "desktop.py",
            base=base,
            icon="ui/static/favicon.ico",
            target_name="PSP.exe",
            shortcut_name="Palworld Save Pal",
            shortcut_dir="ProgramMenuDir",
        )
    ],
)
