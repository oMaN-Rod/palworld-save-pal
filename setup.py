from cx_Freeze import setup, Executable
import sys
from palworld_save_pal.__version__ import __version__

build_exe_options = {
    "include_files": [
        ("ui_build", "ui"),
        ("data", "data"),
    ],
    "packages": ["uvicorn", "fastapi", "webview", "palworld_save_tools", "websockets"],
    "replace_paths": [("*", "")],
}

base = "Win32GUI" if sys.platform == "win32" else None

setup(
    name="PalworldSavePal",
    version=__version__,
    description="Palworld Save Editor",
    options={"build_exe": build_exe_options},
    executables=[
        Executable(
            "desktop.py", base=base, icon="ui/static/favicon.ico", target_name="PSP.exe"
        )
    ],
)
