from typing import Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel, Field

from palworld_save_pal.dto.item_container import ItemContainerDTO


class PlayerDTO(BaseModel):
    uid: UUID
    nickname: str
    level: int
    exp: int
    hp: int = 5000
    stomach: float = 100.0
    sanity: float = 100.0
    status_point_list: Dict[str, int] = Field(default_factory=dict)
    ext_status_point_list: Dict[str, int] = Field(default_factory=dict)
    instance_id: Optional[UUID] = None
    guild_id: Optional[UUID] = None
    pal_box_id: Optional[UUID] = None
    otomo_container_id: Optional[UUID] = None
    common_container: Optional[ItemContainerDTO] = None
    essential_container: Optional[ItemContainerDTO] = None
    weapon_load_out_container: Optional[ItemContainerDTO] = None
    player_equipment_armor_container: Optional[ItemContainerDTO] = None
    food_equip_container: Optional[ItemContainerDTO] = None
    technologies: List[str] = Field(default_factory=list)
    technology_points: int = 0
    boss_technology_points: int = 0
    current_missions: List[str] = Field(default_factory=list)
    completed_missions: List[str] = Field(default_factory=list)
