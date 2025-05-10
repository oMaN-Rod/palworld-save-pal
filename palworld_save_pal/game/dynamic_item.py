from typing import Optional
from uuid import UUID

from pydantic import BaseModel, computed_field

from palworld_save_pal.game.utils import clean_character_id


class DynamicItem(BaseModel):
    local_id: UUID
    type: Optional[str] = None
    durability: Optional[float] = None
    remaining_bullets: Optional[int] = None
    character_id: Optional[str] = None

    @computed_field
    def character_key(self) -> Optional[str]:
        if self.character_id is None:
            return None
        _, character_key = clean_character_id(self.character_id)
        return character_key
