from pydantic import BaseModel


class SettingsDTO(BaseModel):
    language: str
    clone_prefix: str
    new_pal_prefix: str
    debug_mode: bool
    cheat_mode: bool
