from typing import TYPE_CHECKING

from palworld_save_pal.ws.handlers import (
    app_state_handler,
    pal_handler,
    save_file_handler,
)
from palworld_save_pal.ws.messages import (
    GetPalDetailsMessage,
    LoadSaveFileMessage,
    MessageType,
    SyncAppStateMessage,
    UpdateSaveFileMessage,
    DownloadSaveFileMessage,
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
        MessageType.UPDATE_SAVE_FILE.value,
        {
            "message_class": UpdateSaveFileMessage,
            "handler_func": save_file_handler.update_save_file_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_PAL_DETAILS.value,
        {
            "message_class": GetPalDetailsMessage,
            "handler_func": pal_handler.get_pal_details_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.SYNC_APP_STATE.value,
        {
            "message_class": SyncAppStateMessage,
            "handler_func": app_state_handler.sync_app_state_handler,
        },
    )
