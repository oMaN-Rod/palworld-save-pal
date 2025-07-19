from typing import Dict, List, Optional
from uuid import UUID

from pydantic import BaseModel

from palworld_save_pal.dto.base import BaseDTO
from palworld_save_pal.dto.item_container import ItemContainerDTO
from palworld_save_pal.game.guild_lab_research_info import GuildLabResearchInfo


class GuildDTO(BaseModel):
    name: Optional[str] = None
    bases: Optional[Dict[UUID, BaseDTO]] = None
    guild_chest: Optional[ItemContainerDTO] = None
    lab_research: Optional[List[GuildLabResearchInfo]] = None
