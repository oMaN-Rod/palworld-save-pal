from typing import TYPE_CHECKING

from palworld_save_pal.ws.handlers import (
    active_skills_handler,
    app_state_handler,
    elements_handler,
    items_handler,
    passive_skills_handler,
    preset_handler,
    save_file_handler,
    pal_handler,
)
from palworld_save_pal.ws.messages import (
    AddPalMessage,
    AddPresetMessage,
    ClonePalMessage,
    GetActiveSkillsMessage,
    GetElementsMessage,
    GetItemsMessage,
    GetPalsMessage,
    GetPassiveSkillsMessage,
    HealPalsMessage,
    LoadSaveFileMessage,
    MessageType,
    SyncAppStateMessage,
    UpdateSaveFileMessage,
    DownloadSaveFileMessage,
    LoadZipFileMessage,
    DeletePalsMessage,
    GetPresetsMessage,
    DeletePresetMessage,
    UpdatePresetMessage,
)

if TYPE_CHECKING:
    from palworld_save_pal.ws.dispatcher import MessageDispatcher


def bootstrap(dispatcher: "MessageDispatcher"):
    dispatcher.register_handler(
        MessageType.DOWNLOAD_SAVE_FILE.value,
        {
            "message_class": DownloadSaveFileMessage,
            "handler_func": save_file_handler.download_save_file_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.LOAD_SAVE_FILE.value,
        {
            "message_class": LoadSaveFileMessage,
            "handler_func": save_file_handler.load_save_file_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.LOAD_ZIP_FILE.value,
        {
            "message_class": LoadZipFileMessage,
            "handler_func": save_file_handler.load_zip_file_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.UPDATE_SAVE_FILE.value,
        {
            "message_class": UpdateSaveFileMessage,
            "handler_func": save_file_handler.update_save_file_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.SYNC_APP_STATE.value,
        {
            "message_class": SyncAppStateMessage,
            "handler_func": app_state_handler.sync_app_state_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.DELETE_PALS.value,
        {
            "message_class": DeletePalsMessage,
            "handler_func": pal_handler.delete_pals_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.HEAL_PALS.value,
        {
            "message_class": HealPalsMessage,
            "handler_func": pal_handler.heal_pals_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.ADD_PAL.value,
        {
            "message_class": AddPalMessage,
            "handler_func": pal_handler.add_pal_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.CLONE_PAL.value,
        {
            "message_class": ClonePalMessage,
            "handler_func": pal_handler.clone_pal_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.ADD_PRESET.value,
        {
            "message_class": AddPresetMessage,
            "handler_func": preset_handler.add_preset_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_PRESETS.value,
        {
            "message_class": GetPresetsMessage,
            "handler_func": preset_handler.get_presets_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.UPDATE_PRESET.value,
        {
            "message_class": UpdatePresetMessage,
            "handler_func": preset_handler.update_preset_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.DELETE_PRESET.value,
        {
            "message_class": DeletePresetMessage,
            "handler_func": preset_handler.delete_presets_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_ACTIVE_SKILLS.value,
        {
            "message_class": GetActiveSkillsMessage,
            "handler_func": active_skills_handler.get_active_skills_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_PASSIVE_SKILLS.value,
        {
            "message_class": GetPassiveSkillsMessage,
            "handler_func": passive_skills_handler.get_passive_skills_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_ELEMENTS.value,
        {
            "message_class": GetElementsMessage,
            "handler_func": elements_handler.get_elements_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_ITEMS.value,
        {
            "message_class": GetItemsMessage,
            "handler_func": items_handler.get_items_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_PALS.value,
        {
            "message_class": GetPalsMessage,
            "handler_func": pal_handler.get_pals_handler,
        },
    )
