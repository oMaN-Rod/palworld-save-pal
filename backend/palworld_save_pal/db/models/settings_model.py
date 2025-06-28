from sqlmodel import Field, SQLModel
from palworld_save_pal.utils.file_manager import STEAM_ROOT


class SettingsModel(SQLModel, table=True):
    id: int = Field(default=1, primary_key=True)
    language: str = Field(default="en")
    save_dir: str = Field(default=STEAM_ROOT)
    clone_prefix: str = Field(default="©️")
    new_pal_prefix: str = Field(default="🆕")
    debug_mode: bool = Field(default=False)
    cheat_mode: bool = Field(default=False)
