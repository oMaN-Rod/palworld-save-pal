from enum import Enum
from typing import Any, Dict, List, Optional, Union
from pydantic import BaseModel
from uuid import UUID

from palworld_save_pal.dto.preset_profile import PresetProfileDTO
from palworld_save_pal.dto.settings import SettingsDTO
from palworld_save_pal.dto.guild import GuildDTO
from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.dto.player import PlayerDTO
from palworld_save_pal.game.guild import GuildLabResearchInfo


class BaseMessage(BaseModel):
    type: str
    data: Any = None


class MessageType(str, Enum):
    # Pal Management
    ADD_PAL = "add_pal"
    ADD_DPS_PAL = "add_dps_pal"
    ADD_GPS_PAL = "add_gps_pal"
    CLONE_PAL = "clone_pal"
    CLONE_DPS_PAL = "clone_dps_pal"
    CLONE_GPS_PAL = "clone_gps_pal"
    DELETE_PALS = "delete_pals"
    DELETE_DPS_PALS = "delete_dps_pals"
    DELETE_GPS_PALS = "delete_gps_pals"
    GET_PAL_DETAILS = "get_pal_details"  # remove
    GET_PALS = "get_pals"
    GET_GPS_PALS = "get_gps_pals"
    HEAL_ALL_PALS = "heal_all_pals"
    HEAL_PALS = "heal_pals"
    MOVE_PAL = "move_pal"

    # Player Management
    DELETE_PLAYER = "delete_player"
    SET_TECHNOLOGY_DATA = "set_technology_data"

    # Game Data Retrieval
    GET_ACTIVE_SKILLS = "get_active_skills"
    GET_BUILDINGS = "get_buildings"
    GET_ELEMENTS = "get_elements"
    GET_EXP_DATA = "get_exp_data"
    GET_MAP_OBJECTS = "get_map_objects"
    GET_GUILDS = "get_guilds"
    GET_ITEMS = "get_items"
    GET_PASSIVE_SKILLS = "get_passive_skills"
    GET_PLAYERS = "get_players"
    GET_TECHNOLOGIES = "get_technologies"
    GET_UI_COMMON = "get_ui_common"
    GET_VERSION = "get_version"
    GET_WORK_SUITABILITY = "get_work_suitability"
    GET_FRIENDSHIP_DATA = "get_friendship_data"

    # Preset Operations
    ADD_PRESET = "add_preset"
    DELETE_PRESET = "delete_preset"
    GET_PRESETS = "get_presets"
    UPDATE_PRESET = "update_preset"
    EXPORT_PRESET = "export_preset"
    IMPORT_PRESET = "import_preset"

    # Guild Management
    DELETE_GUILD = "delete_guild"
    GET_LAB_RESEARCH = "get_lab_research"
    UPDATE_LAB_RESEARCH = "update_lab_research"

    # Save File Management
    DOWNLOAD_SAVE_FILE = "download_save_file"
    LOADED_SAVE_FILES = "loaded_save_files"
    LOAD_ZIP_FILE = "load_zip_file"
    NO_FILE_SELECTED = "no_file_selected"
    SAVE_MODDED_SAVE = "save_modded_save"
    SELECT_GAMEPASS_SAVE = "select_gamepass_save"
    SELECT_SAVE = "select_save"
    UPDATE_SAVE_FILE = "update_save_file"
    RENAME_WORLD = "rename_world"

    # Settings Management
    GET_SETTINGS = "get_settings"
    UPDATE_SETTINGS = "update_settings"
    NUKE_PRESETS = "nuke_presets"

    # System Messages
    ERROR = "error"
    PROGRESS_MESSAGE = "progress_message"
    SYNC_APP_STATE = "sync_app_state"
    WARNING = "warning"

    # Utility
    OPEN_IN_BROWSER = "open_in_browser"  # remove

    # Debug
    GET_GUILD_RAW_DATA = "get_guild_raw_data"
    GET_RAW_DATA = "get_raw_data"


class AddPalData(BaseModel):
    player_id: Optional[UUID] = None
    guild_id: Optional[UUID] = None
    base_id: Optional[UUID] = None
    character_id: str
    nickname: str
    container_id: Optional[UUID] = None
    storage_slot: Optional[int] = None


class AddPalMessage(BaseMessage):
    type: str = MessageType.ADD_PAL.value
    data: AddPalData


class AddDpsPalMessage(BaseMessage):
    type: str = MessageType.ADD_DPS_PAL.value
    data: AddPalData


class MovePalData(BaseModel):
    player_id: UUID
    pal_id: UUID
    container_id: UUID


class MovePalMessage(BaseMessage):
    type: str = MessageType.MOVE_PAL.value
    data: MovePalData


class ClonePalData(BaseModel):
    pal: PalDTO
    guild_id: Optional[UUID] = None
    base_id: Optional[UUID] = None


class ClonePalMessage(BaseMessage):
    type: str = MessageType.CLONE_PAL.value
    data: ClonePalData


class CloneDpsPalMessage(BaseMessage):
    type: str = MessageType.CLONE_DPS_PAL.value
    data: ClonePalData


class DeletePalsData(BaseModel):
    pal_indexes: Optional[List[int]] = None
    pal_ids: Optional[List[UUID]] = None
    player_id: Optional[UUID] = None
    guild_id: Optional[UUID] = None
    base_id: Optional[UUID] = None


class DeletePalsMessage(BaseMessage):
    type: str = MessageType.DELETE_PALS.value
    data: DeletePalsData


class DeleteDpsPalsMessage(BaseMessage):
    type: str = MessageType.DELETE_DPS_PALS.value
    data: DeletePalsData


class HealPalsMessage(BaseMessage):
    type: str = MessageType.HEAL_PALS.value
    data: List[UUID]


class DownloadSaveFileMessage(BaseMessage):
    type: str = MessageType.DOWNLOAD_SAVE_FILE.value


class UpdateSaveFileData(BaseModel):
    modified_pals: Optional[Dict[UUID, PalDTO]] = None
    modified_dps_pals: Optional[Dict[int, PalDTO]] = None
    modified_players: Optional[Dict[UUID, PlayerDTO]] = None
    modified_guilds: Optional[Dict[UUID, GuildDTO]] = None
    modified_gps_pals: Optional[Dict[int, PalDTO]] = None


class UpdateSaveFileMessage(BaseMessage):
    type: str = MessageType.UPDATE_SAVE_FILE.value
    data: UpdateSaveFileData


class GetPalDetailsMessage(BaseMessage):
    type: str = MessageType.GET_PAL_DETAILS.value
    data: UUID


class SyncAppStateMessage(BaseMessage):
    type: str = MessageType.SYNC_APP_STATE.value
    data: None = None


class ProgressMessage(BaseMessage):
    type: str = MessageType.PROGRESS_MESSAGE.value
    data: str


class LoadZipFileMessage(BaseMessage):
    type: str = MessageType.LOAD_ZIP_FILE.value
    data: List[int]


class GetPresetsMessage(BaseMessage):
    type: str = MessageType.GET_PRESETS.value


class UpdatePresetData(BaseModel):
    id: UUID
    name: str


class UpdatePresetMessage(BaseMessage):
    type: str = MessageType.UPDATE_PRESET.value
    data: UpdatePresetData


class AddPresetMessage(BaseMessage):
    type: str = MessageType.ADD_PRESET.value
    data: PresetProfileDTO


class DeletePresetMessage(BaseMessage):
    type: str = MessageType.DELETE_PRESET.value
    data: List[str]


class GetActiveSkillsMessage(BaseModel):
    type: str = MessageType.GET_ACTIVE_SKILLS.value


class GetPassiveSkillsMessage(BaseModel):
    type: str = MessageType.GET_PASSIVE_SKILLS.value


class GetElementsMessage(BaseModel):
    type: str = MessageType.GET_ELEMENTS.value


class GetItemsMessage(BaseModel):
    type: str = MessageType.GET_ITEMS.value


class GetPalsMessage(BaseModel):
    type: str = MessageType.GET_PALS.value


class OpenInBrowserMessage(BaseMessage):
    type: str = MessageType.OPEN_IN_BROWSER.value
    data: str


class GetVersionMessage(BaseModel):
    type: str = MessageType.GET_VERSION.value


class SelectSaveMessageData(BaseModel):
    type: str
    path: str
    local: bool


class SelectSaveMessage(BaseMessage):
    type: str = MessageType.SELECT_SAVE.value
    data: SelectSaveMessageData


class SaveModdedSaveMessage(BaseMessage):
    type: str = MessageType.SAVE_MODDED_SAVE.value
    data: str


class GetSettingsMessage(BaseMessage):
    type: str = MessageType.GET_SETTINGS.value


class UpdateSettingsMessage(BaseMessage):
    type: str = MessageType.UPDATE_SETTINGS.value
    data: SettingsDTO


class GetUICommonMessage(BaseMessage):
    type: str = MessageType.GET_UI_COMMON.value


class SelectGamepassSaveMessage(BaseMessage):
    type: str = MessageType.SELECT_GAMEPASS_SAVE.value
    data: str


class GetWorkSuitabilityMessage(BaseMessage):
    type: str = MessageType.GET_WORK_SUITABILITY.value


class HealAllPalData(BaseModel):
    player_id: Optional[UUID] = None
    guild_id: Optional[UUID] = None
    base_id: Optional[UUID] = None


class HealAllPalsMessage(BaseMessage):
    type: str = MessageType.HEAL_ALL_PALS.value
    data: HealAllPalData


class GetBuildingsMessage(BaseModel):
    type: str = MessageType.GET_BUILDINGS.value


class GetRawDataData(BaseModel):
    guild_id: Optional[UUID] = None
    player_id: Optional[UUID] = None
    pal_id: Optional[UUID] = None
    base_id: Optional[UUID] = None
    item_container_id: Optional[UUID] = None
    character_container_id: Optional[UUID] = None
    level: Optional[bool] = False


class GetRawDataMessage(BaseModel):
    type: str = MessageType.GET_RAW_DATA.value
    data: GetRawDataData


class GetTechnologiesMessage(BaseModel):
    type: str = MessageType.GET_TECHNOLOGIES.value


class TechnologyData(BaseModel):
    playerID: UUID = None
    technologies: List[str] = None
    techPoints: int = None
    ancientTechPoints: int = None


class SetTechnologyDataMessage(BaseModel):
    type: str = MessageType.SET_TECHNOLOGY_DATA.value
    data: TechnologyData


class GetMapObjectsMessage(BaseModel):
    type: str = MessageType.GET_MAP_OBJECTS.value


class DeleteGuildData(BaseModel):
    guild_id: UUID
    origin: str


class DeleteGuildMessage(BaseModel):
    type: str = MessageType.DELETE_GUILD.value
    data: DeleteGuildData


class DeletePlayerData(BaseModel):
    player_id: UUID
    origin: str


class DeletePlayerMessage(BaseModel):
    type: str = MessageType.DELETE_PLAYER.value
    data: DeletePlayerData


class NukePresetsMessage(BaseMessage):
    type: str = MessageType.NUKE_PRESETS.value


class GetLabResearchMessage(BaseMessage):
    type: str = MessageType.GET_LAB_RESEARCH.value


class UpdateLabResearchData(BaseModel):
    guild_id: UUID
    research_updates: List[GuildLabResearchInfo]


class UpdateLabResearchMessage(BaseMessage):
    type: str = MessageType.UPDATE_LAB_RESEARCH.value
    data: UpdateLabResearchData


class ExportPresetData(BaseModel):
    preset_id: str
    preset_type: str
    preset_name: str


class ExportPresetMessage(BaseMessage):
    type: str = MessageType.EXPORT_PRESET.value
    data: ExportPresetData


class ImportPresetMessage(BaseMessage):
    type: str = MessageType.IMPORT_PRESET.value


class GetGpsMessage(BaseModel):
    type: str = MessageType.GET_GPS_PALS.value


class AddGpsPalMessage(BaseMessage):
    type: str = MessageType.ADD_DPS_PAL.value
    data: AddPalData


class DeleteGpsPalsMessage(BaseMessage):
    type: str = MessageType.DELETE_DPS_PALS.value
    data: DeletePalsData


class RenameWorldMessage(BaseMessage):
    type: str = MessageType.RENAME_WORLD.value
    data: str
