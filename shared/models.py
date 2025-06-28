# Shared Pydantic models for WebSocket messages
from pydantic import BaseModel
from enum import Enum
from typing import Any, Dict, List, Optional, Union

class MessageType(str, Enum):
    ADD_PAL = "add_pal"
    ADD_DPS_PAL = "add_dps_pal"
    CLONE_PAL = "clone_pal"
    CLONE_DPS_PAL = "clone_dps_pal"
    DELETE_PALS = "delete_pals"
    DELETE_DPS_PALS = "delete_dps_pals"
    HEAL_PALS = "heal_pals"
    HEAL_ALL_PALS = "heal_all_pals"
    DOWNLOAD_SAVE_FILE = "download_save_file"
    ERROR = "error"
    WARNING = "warning"
    GET_GUILDS = "get_guilds"
    GET_PLAYERS = "get_players"
    GET_PAL_DETAILS = "get_pal_details"
    LOAD_ZIP_FILE = "load_zip_file"
    PROGRESS_MESSAGE = "progress_message"
    SYNC_APP_STATE = "sync_app_state"
    UPDATE_SAVE_FILE = "update_save_file"
    GET_PRESETS = "get_presets"
    ADD_PRESET = "add_preset"
    UPDATE_PRESET = "update_preset"
    DELETE_PRESET = "delete_preset"
    GET_ACTIVE_SKILLS = "get_active_skills"
    GET_PASSIVE_SKILLS = "get_passive_skills"
    GET_TECHNOLOGIES = "get_technologies"
    GET_ELEMENTS = "get_elements"
    GET_ITEMS = "get_items"
    GET_PALS = "get_pals"
    SET_TECHNOLOGY_DATA = "set_technology_data"
    OPEN_IN_BROWSER = "open_in_browser"
    GET_EXP_DATA = "get_exp_data"
    GET_VERSION = "get_version"
    SELECT_SAVE = "select_save"
    LOADED_SAVE_FILES = "loaded_save_files"
    SAVE_MODDED_SAVE = "save_modded_save"
    GET_SETTINGS = "get_settings"
    UPDATE_SETTINGS = "update_settings"
    GET_UI_COMMON = "get_ui_common"
    NO_FILE_SELECTED = "no_file_selected"
    SELECT_GAMEPASS_SAVE = "select_gamepass_save"
    GET_SAVE_TYPE = "get_save_type"
    GET_WORK_SUITABILITY = "get_work_suitability"
    GET_BUILDINGS = "get_buildings"
    GET_RAW_DATA = "get_raw_data"
    GET_MAP_OBJECTS = "get_map_objects"
    DELETE_GUILD = "delete_guild"
    DELETE_PLAYER = "delete_player"
    NUKE_PRESETS = "nuke_presets"
    GET_LAB_RESEARCH = "get_lab_research"
    UPDATE_LAB_RESEARCH = "update_lab_research"

class BaseMessage(BaseModel):
    type: str
    data: Any = None

class SettingsDTO(BaseModel):
    language: str
    clone_prefix: str
    new_pal_prefix: str
    debug_mode: bool
    cheat_mode: bool

class UpdateSettingsMessage(BaseMessage):
    type: str = MessageType.UPDATE_SETTINGS.value
    data: SettingsDTO

class GetWorkSuitabilityMessage(BaseMessage):
    type: str = MessageType.GET_WORK_SUITABILITY.value

# --- Shared domain models ---

class EntryState(str, Enum):
    NONE = 'None'
    MODIFIED = 'Modified'
    NEW = 'New'
    DELETED = 'Deleted'

class PalGender(str, Enum):
    MALE = 'Male'
    FEMALE = 'Female'

class WorkSuitability(str, Enum):
    EmitFlame = 'EmitFlame'
    Watering = 'Watering'
    Seeding = 'Seeding'
    GenerateElectricity = 'GenerateElectricity'
    Handcraft = 'Handcraft'
    Collection = 'Collection'
    Deforest = 'Deforest'
    Mining = 'Mining'
    OilExtraction = 'OilExtraction'
    ProductMedicine = 'ProductMedicine'
    Cool = 'Cool'
    Transport = 'Transport'
    MonsterFarm = 'MonsterFarm'

class ElementType(str, Enum):
    Fire = 'Fire'
    Water = 'Water'
    Ground = 'Ground'
    Ice = 'Ice'
    Neutral = 'Neutral'
    Dark = 'Dark'
    Grass = 'Grass'
    Dragon = 'Dragon'
    Electric = 'Electric'

class StatusPointList(BaseModel):
    max_hp: int
    max_sp: int
    attack: int
    weight: int
    capture_rate: int
    work_speed: int

class ExStatusPointList(BaseModel):
    max_hp: int
    max_sp: int
    attack: int
    weight: int
    work_speed: int

class WorldMapPoint(BaseModel):
    x: float
    y: float
    z: float

class GamePassContainer(BaseModel):
    path: str
    guid: str
    num: int
    name: str

class GamepassSave(BaseModel):
    save_id: str
    world_name: str
    player_count: int
    containers: list[GamePassContainer]

class EggConfig(BaseModel):
    character_id: str
    gender: PalGender
    talent_hp: int
    talent_shot: int
    talent_defense: int
    learned_skills: list[str]
    active_skills: list[str]
    passive_skills: list[str]

class CharacterContainerSlot(BaseModel):
    slot_index: int
    pal_id: Optional[str] = None

class CharacterContainerType(str, Enum):
    PAL_BOX = 'PalBox'
    PARTY = 'Party'
    BASE = 'Base'

class CharacterContainer(BaseModel):
    id: str
    player_uid: str
    type: CharacterContainerType
    size: Optional[int] = None
    slots: Optional[list[CharacterContainerSlot]] = None

class Pal(BaseModel):
    name: str
    instance_id: str
    owner_uid: str
    character_id: str
    character_key: str
    is_lucky: bool
    is_boss: bool
    is_predator: bool
    gender: PalGender
    rank_hp: int
    rank_attack: int
    rank_defense: int
    rank_craftspeed: int
    talent_hp: int
    talent_shot: int
    talent_defense: int
    rank: int
    level: int
    nickname: Optional[str] = None
    is_tower: bool
    stomach: int
    storage_id: Optional[str] = None
    storage_slot: int
    learned_skills: list[str]
    active_skills: list[str]
    passive_skills: list[str]
    work_suitability: dict[WorkSuitability, int]
    hp: int
    max_hp: int
    elements: list[ElementType]
    state: EntryState
    sanity: int
    exp: int
    is_sick: bool

class Player(BaseModel):
    uid: str
    nickname: str
    level: int
    hp: int
    pals: Optional[dict[str, 'Pal']] = None
    dps: Optional[dict[int, 'Pal']] = None
    pal_box_id: str
    pal_box: 'CharacterContainer'
    otomo_container_id: str
    party: 'CharacterContainer'
    common_container: 'ItemContainer'
    essential_container: 'ItemContainer'
    weapon_load_out_container: 'ItemContainer'
    player_equipment_armor_container: 'ItemContainer'
    food_equip_container: 'ItemContainer'
    state: EntryState
    exp: int
    stomach: int
    sanity: int
    status_point_list: StatusPointList
    ex_status_point_list: ExStatusPointList
    guild_id: str
    technologies: list[str]
    technology_points: int
    boss_technology_points: int
    location: WorldMapPoint
    last_online_time: str

class DynamicItem(BaseModel):
    local_id: str
    durability: int
    remaining_bullets: Optional[int] = None
    type: str  # DynamicItemClass, can be refined
    character_id: Optional[str] = None
    character_key: Optional[str] = None
    gender: str
    talent_hp: int
    talent_shot: int
    talent_defense: int
    learned_skills: list[str]
    active_skills: list[str]
    passive_skills: list[str]
    modified: bool

class ItemContainerSlot(BaseModel):
    slot_index: int
    static_id: str
    count: int
    dynamic_item: Optional[DynamicItem] = None

class ItemContainer(BaseModel):
    id: str
    type: str
    slots: list[ItemContainerSlot]
    key: str
    slot_num: int
    state: Optional[EntryState] = None

class GuildLabResearchInfo(BaseModel):
    research_id: str
    work_amount: int

class BaseDTO(BaseModel):
    id: str
    storage_containers: dict[str, ItemContainer]

class GuildDTO(BaseModel):
    name: Optional[str] = None
    base: Optional[BaseDTO] = None
    guild_chest: Optional[ItemContainer] = None
    lab_research: Optional[list[GuildLabResearchInfo]] = None

class SaveFileType(str, Enum):
    GAMEPASS = 'gamepass'
    STEAM = 'steam'

class SaveFile(BaseModel):
    name: str
    type: SaveFileType
    world_name: Optional[str] = None
    size: Optional[int] = None

class Base(BaseModel):
    id: str
    name: Optional[str] = None
    pals: dict[str, Pal]
    container_id: str
    pal_container: CharacterContainer
    slot_count: int
    storage_containers: dict[str, ItemContainer]
    state: EntryState
    location: WorldMapPoint

class Guild(BaseModel):
    admin_player_uid: str
    bases: dict[str, Base]
    id: str
    name: str
    players: list[str]
    container_id: Optional[str] = None
    guild_chest: Optional[ItemContainer] = None
    lab_research_data: Optional[list[GuildLabResearchInfo]] = None
    state: EntryState

class MapObject(BaseModel):
    x: float
    y: float
    z: float
    type: str
    localized_name: str
    pal: str

# Forward refs for all models that reference each other
Player.update_forward_refs()
Base.update_forward_refs()
Guild.update_forward_refs()
CharacterContainer.update_forward_refs()

# Add more domain models here (Pal, Player, Guild, Base, Item, etc.)

class Message(BaseModel):
    type: str
    data: Any = None

# MessageType enum is already defined as MessageType in this file and will be exported.
