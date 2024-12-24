from enum import Enum
from typing import Any, Dict, List, Optional
from pydantic import BaseModel
from uuid import UUID

from palworld_save_pal.editor.preset_profile import PresetProfile
from palworld_save_pal.editor.settings import Settings
from palworld_save_pal.game.pal import Pal
from palworld_save_pal.game.player import Player


class BaseMessage(BaseModel):
    type: str
    data: None = None


class MessageType(str, Enum):
    ADD_PAL = "add_pal"
    MOVE_PAL = "move_pal"
    CLONE_PAL = "clone_pal"
    DELETE_PALS = "delete_pals"
    HEAL_PALS = "heal_pals"
    DOWNLOAD_SAVE_FILE = "download_save_file"
    ERROR = "error"
    WARNING = "warning"
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
    GET_ELEMENTS = "get_elements"
    GET_ITEMS = "get_items"
    GET_PALS = "get_pals"
    OPEN_IN_BROWSER = "open_in_browser"
    GET_EXP_DATA = "get_exp_data"
    GET_VERSION = "get_version"
    SELECT_SAVE = "select_save"
    LOADED_SAVE_FILES = "loaded_save_files"
    SAVE_MODDED_SAVE = "save_modded_save"
    GET_SETTINGS = "get_settings"
    UPDATE_SETTINGS = "update_settings"
    GET_UI_COMMON = "get_ui_common"


class AddPalData(BaseModel):
    player_id: UUID
    pal_code_name: str
    nickname: str
    container_id: UUID


class MovePalData(BaseModel):
    player_id: UUID
    pal_id: UUID
    container_id: UUID


class AddPalMessage(BaseMessage):
    type: str = MessageType.ADD_PAL.value
    data: AddPalData


class MovePalMessage(BaseMessage):
    type: str = MessageType.MOVE_PAL.value
    data: MovePalData


class ClonePalMessage(BaseMessage):
    type: str = MessageType.CLONE_PAL.value
    data: Pal


class DeletePalsData(BaseModel):
    player_id: UUID
    pal_ids: List[UUID]


class DeletePalsMessage(BaseMessage):
    type: str = MessageType.DELETE_PALS.value
    data: DeletePalsData


class HealPalsMessage(BaseMessage):
    type: str = MessageType.HEAL_PALS.value
    data: List[UUID]


class DownloadSaveFileMessage(BaseMessage):
    type: str = MessageType.DOWNLOAD_SAVE_FILE.value


class UpdateSaveFileData(BaseModel):
    modified_pals: Optional[Dict[UUID, Pal]] = None
    modified_players: Optional[Dict[UUID, Player]] = None


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
    data: PresetProfile


class DeletePresetMessage(BaseMessage):
    type: str = MessageType.DELETE_PRESET.value
    data: List[UUID]


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


class GetSettingsMessage(BaseMessage):
    type: str = MessageType.GET_SETTINGS.value


class UpdateSettingsMessage(BaseMessage):
    type: str = MessageType.UPDATE_SETTINGS.value
    data: Settings


class GetUICommonMessage(BaseMessage):
    type: str = MessageType.GET_UI_COMMON.value
