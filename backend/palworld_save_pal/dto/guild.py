from typing import List, Optional

from pydantic import BaseModel

from palworld_save_pal.dto.base import BaseDTO
from palworld_save_pal.dto.item_container import ItemContainerDTO
from palworld_save_pal.game.guild_lab_research_info import GuildLabResearchInfo


class GuildDTO(BaseModel):
    name: Optional[str] = None
    base: Optional[BaseDTO] = None
    guild_chest: Optional[ItemContainerDTO] = None
    lab_research: Optional[List[GuildLabResearchInfo]] = None
