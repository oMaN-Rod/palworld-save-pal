from typing import TYPE_CHECKING

from palworld_save_pal.ws.handlers import (
    active_skills_handler,
    app_state_handler,
    buildings_handler,
    debug_handler,
    elements_handler,
    exp_handler,
    friendship_handler,
    guild_handler,
    items_handler,
    lab_research_handler,
    lazy_load_handler,
    map_objects_handler,
    map_unlock_handler,
    open_in_browser_handler,
    passive_skills_handler,
    player_handler,
    technologies_handler,
    preset_handler,
    save_file_handler,
    pal_handler,
    settings_handler,
    ui_common_handler,
    version_handler,
    local_file_handler,
    work_suitability_handler,
    gps_handler,
    ups_handler,
)
from palworld_save_pal.ws.messages import (
    AddDpsPalMessage,
    AddPalMessage,
    AddPresetMessage,
    BaseMessage,
    CloneDpsPalMessage,
    ClonePalMessage,
    DeleteDpsPalsMessage,
    DeleteGpsPalsMessage,
    DeleteGuildMessage,
    DeletePlayerMessage,
    ExportPresetMessage,
    GetActiveSkillsMessage,
    GetBuildingsMessage,
    GetElementsMessage,
    GetItemsMessage,
    GetLabResearchMessage,
    GetMapObjectsMessage,
    GetPalsMessage,
    GetPassiveSkillsMessage,
    GetRawDataMessage,
    GetSettingsMessage,
    GetTechnologiesMessage,
    GetWorkSuitabilityMessage,
    HealAllPalsMessage,
    HealPalsMessage,
    ImportPresetMessage,
    MessageType,
    MovePalMessage,
    NukePresetsMessage,
    OpenInBrowserMessage,
    RenameWorldMessage,
    SelectGamepassSaveMessage,
    SetTechnologyDataMessage,
    SyncAppStateMessage,
    UpdateLabResearchMessage,
    UpdateSaveFileMessage,
    DownloadSaveFileMessage,
    LoadZipFileMessage,
    DeletePalsMessage,
    GetPresetsMessage,
    DeletePresetMessage,
    UpdatePresetMessage,
    GetVersionMessage,
    SelectSaveMessage,
    UpdateSettingsMessage,
    AddGpsPalMessage,
    # UPS Messages
    GetUpsPalsMessage,
    GetUpsAllFilteredIdsMessage,
    AddUpsPalMessage,
    UpdateUpsPalMessage,
    DeleteUpsPalsMessage,
    CloneUpsPalMessage,
    CloneToUpsMessage,
    ExportUpsPalMessage,
    ImportToUpsMessage,
    GetUpsCollectionsMessage,
    CreateUpsCollectionMessage,
    UpdateUpsCollectionMessage,
    DeleteUpsCollectionMessage,
    GetUpsTagsMessage,
    CreateUpsTagMessage,
    UpdateUpsTagMessage,
    DeleteUpsTagMessage,
    GetUpsStatsMessage,
    NukeUpsPalsMessage,
    UnlockMapMessage,
    RequestPlayerDetailsMessage,
    RequestGuildDetailsMessage,
    RequestGpsMessage,
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
        MessageType.DELETE_DPS_PALS.value,
        {
            "message_class": DeleteDpsPalsMessage,
            "handler_func": pal_handler.delete_dps_pals_handler,
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
        MessageType.ADD_DPS_PAL.value,
        {
            "message_class": AddDpsPalMessage,
            "handler_func": pal_handler.add_dps_pal_handler,
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
        MessageType.CLONE_DPS_PAL.value,
        {
            "message_class": CloneDpsPalMessage,
            "handler_func": pal_handler.clone_dps_pal_handler,
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
        MessageType.NUKE_PRESETS.value,
        {
            "message_class": NukePresetsMessage,
            "handler_func": preset_handler.nuke_presets_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.EXPORT_PRESET.value,
        {
            "message_class": ExportPresetMessage,
            "handler_func": preset_handler.export_preset_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.IMPORT_PRESET.value,
        {
            "message_class": ImportPresetMessage,
            "handler_func": preset_handler.import_preset_handler,
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
        MessageType.GET_TECHNOLOGIES.value,
        {
            "message_class": GetTechnologiesMessage,
            "handler_func": technologies_handler.get_technologies_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.SET_TECHNOLOGY_DATA.value,
        {
            "message_class": SetTechnologyDataMessage,
            "handler_func": technologies_handler.set_technology_data_handler,
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

    dispatcher.register_handler(
        MessageType.DELETE_GUILD.value,
        {
            "message_class": DeleteGuildMessage,
            "handler_func": guild_handler.delete_guild_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.OPEN_IN_BROWSER.value,
        {
            "message_class": OpenInBrowserMessage,
            "handler_func": open_in_browser_handler.open_in_browser_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_EXP_DATA.value,
        {
            "message_class": BaseMessage,
            "handler_func": exp_handler.get_exp_data_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_FRIENDSHIP_DATA.value,
        {
            "message_class": BaseMessage,
            "handler_func": friendship_handler.get_friendship_data_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.MOVE_PAL.value,
        {
            "message_class": MovePalMessage,
            "handler_func": pal_handler.move_pal_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_VERSION.value,
        {
            "message_class": GetVersionMessage,
            "handler_func": version_handler.get_version_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.SELECT_SAVE.value,
        {
            "message_class": SelectSaveMessage,
            "handler_func": local_file_handler.select_save_files_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.SAVE_MODDED_SAVE.value,
        {
            "message_class": BaseMessage,
            "handler_func": local_file_handler.save_modded_save_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.SELECT_GAMEPASS_SAVE.value,
        {
            "message_class": SelectGamepassSaveMessage,
            "handler_func": local_file_handler.select_gamepass_save_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_SETTINGS.value,
        {
            "message_class": GetSettingsMessage,
            "handler_func": settings_handler.get_settings_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.UPDATE_SETTINGS.value,
        {
            "message_class": UpdateSettingsMessage,
            "handler_func": settings_handler.update_settings_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_UI_COMMON.value,
        {
            "message_class": BaseMessage,
            "handler_func": ui_common_handler.get_ui_common_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_WORK_SUITABILITY.value,
        {
            "message_class": GetWorkSuitabilityMessage,
            "handler_func": work_suitability_handler.get_work_suitability_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.HEAL_ALL_PALS.value,
        {
            "message_class": HealAllPalsMessage,
            "handler_func": pal_handler.heal_all_pals_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_BUILDINGS.value,
        {
            "message_class": GetBuildingsMessage,
            "handler_func": buildings_handler.get_buildings_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_RAW_DATA.value,
        {
            "message_class": GetRawDataMessage,
            "handler_func": debug_handler.get_raw_data_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_MAP_OBJECTS.value,
        {
            "message_class": GetMapObjectsMessage,
            "handler_func": map_objects_handler.get_map_objects_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.DELETE_PLAYER.value,
        {
            "message_class": DeletePlayerMessage,
            "handler_func": player_handler.delete_player_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_LAB_RESEARCH.value,
        {
            "message_class": GetLabResearchMessage,
            "handler_func": lab_research_handler.get_lab_research_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.UPDATE_LAB_RESEARCH.value,
        {
            "message_class": UpdateLabResearchMessage,
            "handler_func": lab_research_handler.update_lab_research_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.ADD_GPS_PAL.value,
        {
            "message_class": AddGpsPalMessage,
            "handler_func": gps_handler.add_gps_pal_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.DELETE_GPS_PALS.value,
        {
            "message_class": DeleteGpsPalsMessage,
            "handler_func": gps_handler.delete_gps_pals_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.RENAME_WORLD.value,
        {
            "message_class": RenameWorldMessage,
            "handler_func": local_file_handler.rename_world_handler,
        },
    )

    # UPS (Universal Pal Storage) Handlers
    dispatcher.register_handler(
        MessageType.GET_UPS_PALS.value,
        {
            "message_class": GetUpsPalsMessage,
            "handler_func": ups_handler.get_ups_pals_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_UPS_ALL_FILTERED_IDS.value,
        {
            "message_class": GetUpsAllFilteredIdsMessage,
            "handler_func": ups_handler.get_ups_all_filtered_ids_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.ADD_UPS_PAL.value,
        {
            "message_class": AddUpsPalMessage,
            "handler_func": ups_handler.add_ups_pal_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.UPDATE_UPS_PAL.value,
        {
            "message_class": UpdateUpsPalMessage,
            "handler_func": ups_handler.update_ups_pal_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.DELETE_UPS_PALS.value,
        {
            "message_class": DeleteUpsPalsMessage,
            "handler_func": ups_handler.delete_ups_pals_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.CLONE_UPS_PAL.value,
        {
            "message_class": CloneUpsPalMessage,
            "handler_func": ups_handler.clone_ups_pal_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.CLONE_TO_UPS.value,
        {
            "message_class": CloneToUpsMessage,
            "handler_func": ups_handler.clone_to_ups_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.EXPORT_UPS_PAL.value,
        {
            "message_class": ExportUpsPalMessage,
            "handler_func": ups_handler.export_ups_pal_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.IMPORT_TO_UPS.value,
        {
            "message_class": ImportToUpsMessage,
            "handler_func": ups_handler.import_to_ups_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_UPS_COLLECTIONS.value,
        {
            "message_class": GetUpsCollectionsMessage,
            "handler_func": ups_handler.get_ups_collections_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.CREATE_UPS_COLLECTION.value,
        {
            "message_class": CreateUpsCollectionMessage,
            "handler_func": ups_handler.create_ups_collection_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.UPDATE_UPS_COLLECTION.value,
        {
            "message_class": UpdateUpsCollectionMessage,
            "handler_func": ups_handler.update_ups_collection_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.DELETE_UPS_COLLECTION.value,
        {
            "message_class": DeleteUpsCollectionMessage,
            "handler_func": ups_handler.delete_ups_collection_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_UPS_TAGS.value,
        {
            "message_class": GetUpsTagsMessage,
            "handler_func": ups_handler.get_ups_tags_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.CREATE_UPS_TAG.value,
        {
            "message_class": CreateUpsTagMessage,
            "handler_func": ups_handler.create_ups_tag_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.UPDATE_UPS_TAG.value,
        {
            "message_class": UpdateUpsTagMessage,
            "handler_func": ups_handler.update_ups_tag_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.DELETE_UPS_TAG.value,
        {
            "message_class": DeleteUpsTagMessage,
            "handler_func": ups_handler.delete_ups_tag_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.GET_UPS_STATS.value,
        {
            "message_class": GetUpsStatsMessage,
            "handler_func": ups_handler.get_ups_stats_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.NUKE_UPS_PALS.value,
        {
            "message_class": NukeUpsPalsMessage,
            "handler_func": ups_handler.nuke_ups_pals_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.UNLOCK_MAP.value,
        {
            "message_class": UnlockMapMessage,
            "handler_func": map_unlock_handler.unlock_map_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.REQUEST_PLAYER_DETAILS.value,
        {
            "message_class": RequestPlayerDetailsMessage,
            "handler_func": lazy_load_handler.request_player_details_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.REQUEST_GUILD_DETAILS.value,
        {
            "message_class": RequestGuildDetailsMessage,
            "handler_func": lazy_load_handler.request_guild_details_handler,
        },
    )

    dispatcher.register_handler(
        MessageType.REQUEST_GPS.value,
        {
            "message_class": RequestGpsMessage,
            "handler_func": gps_handler.request_gps_handler,
        },
    )
