from typing import TYPE_CHECKING

from palworld_save_pal.ws.handlers import (
    app_state_handler,
    save_file_handler,
    pal_handler,
)
from palworld_save_pal.ws.messages import (
    AddPalMessage,
    ClonePalMessage,
    LoadSaveFileMessage,
    MessageType,
    SyncAppStateMessage,
    UpdateSaveFileMessage,
    DownloadSaveFileMessage,
    LoadZipFileMessage,
    DeletePalsMessage,
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
