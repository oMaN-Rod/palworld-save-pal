from typing import Dict
from uuid import UUID

from pydantic import BaseModel
from palworld_save_pal.dto.item_container import ItemContainerDTO


class BaseDTO(BaseModel):
    id: UUID
    storage_containers: Dict[UUID, ItemContainerDTO]
    name: str | None = None
