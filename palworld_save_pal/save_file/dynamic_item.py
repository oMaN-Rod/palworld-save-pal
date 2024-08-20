from typing import Optional
from uuid import UUID

from pydantic import BaseModel, Field

from palworld_save_pal.save_file.encoders import custom_uuid_encoder


class DynamicItem(BaseModel):
    local_id: UUID = Field(..., json_encoder=custom_uuid_encoder)
    type: Optional[str] = None
    durability: Optional[float] = None
    remaining_bullets: Optional[int] = None
