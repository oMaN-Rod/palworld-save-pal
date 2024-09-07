from enum import Enum
from typing import Any, Dict, List, Optional
from pydantic import BaseModel
from uuid import UUID

from palworld_save_pal.presets.player_preset import PlayerPreset
from palworld_save_pal.save_file.pal import Pal
from palworld_save_pal.save_file.player import Player


class BaseMessage(BaseModel):
    type: str
    data: None = None


class MessageType(str, Enum):
    ADD_PAL = "add_pal"
    CLONE_PAL = "clone_pal"
    DELETE_PALS = "delete_pals"
    HEAL_PALS = "heal_pals"
    DOWNLOAD_SAVE_FILE = "download_save_file"
    ERROR = "error"
    GET_PLAYERS = "get_players"
    GET_PAL_DETAILS = "get_pal_details"
    LOAD_SAVE_FILE = "load_save_file"
    LOAD_ZIP_FILE = "load_zip_file"
    PROGRESS_MESSAGE = "progress_message"
    SYNC_APP_STATE = "sync_app_state"
    UPDATE_SAVE_FILE = "update_save_file"
    GET_PRESETS = "get_presets"
    ADD_PRESET = "add_preset"
    UPDATE_PRESET = "update_preset"
    DELETE_PRESET = "delete_preset"
    GET_ACTIVE_SKILLS = "get_active_skills"


class AddPalData(BaseModel):
    player_id: UUID
    pal_code_name: str
    nickname: str


class AddPalMessage(BaseMessage):
    type: str = MessageType.ADD_PAL.value
    data: AddPalData


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


class LoadSaveFileMessage(BaseMessage):
    type: str = MessageType.LOAD_SAVE_FILE.value
    data: List[int]


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
    data: PlayerPreset


class DeletePresetMessage(BaseMessage):
    type: str = MessageType.DELETE_PRESET.value
    data: List[UUID]


class GetActiveSkillsMessage(BaseModel):
    type: str = MessageType.GET_ACTIVE_SKILLS.value
