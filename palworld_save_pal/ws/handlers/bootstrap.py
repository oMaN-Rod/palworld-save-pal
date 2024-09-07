from typing import TYPE_CHECKING

from palworld_save_pal.ws.handlers import (
    app_state_handler,
    preset_handler,
    save_file_handler,
    pal_handler,
)
from palworld_save_pal.ws.messages import (
    AddPalMessage,
    AddPresetMessage,
    ClonePalMessage,
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
