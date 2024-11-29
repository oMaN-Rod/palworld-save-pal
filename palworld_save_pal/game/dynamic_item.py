from typing import Optional
from uuid import UUID

from pydantic import BaseModel, Field


class DynamicItem(BaseModel):
    local_id: UUID
    type: Optional[str] = None
    durability: Optional[float] = None
    remaining_bullets: Optional[int] = None
