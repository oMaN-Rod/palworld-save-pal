//! UPS pal handlers: get / get_ids / add / update / delete / clone / stats /
//! nuke, plus collections, tags, and save-session interop (clone_to_ups /
//! import_to_ups / export_ups_pal). Backed by the psp-db UPS layer.

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use psp_core::domain::pal;
use psp_core::domain::player::build_player_dto;
use psp_core::dto::pal::PalDto;
use psp_core::session::SaveSession;
use psp_db::ups::{PalTypeFilter, UpsFilter};

fn default_limit() -> i64 {
    30
}
fn default_sort_by() -> String {
    "created_at".to_string()
}
fn default_sort_order() -> String {
    "desc".to_string()
}

#[derive(Debug, serde::Deserialize)]
pub struct GetUpsPalsData {
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub search_query: Option<String>,
    pub character_id_filter: Option<String>,
    pub collection_id: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub element_types: Option<Vec<String>>,
    pub pal_types: Option<Vec<String>>,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct GetUpsAllFilteredIdsData {
    pub search_query: Option<String>,
    pub character_id_filter: Option<String>,
    pub collection_id: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub element_types: Option<Vec<String>>,
    pub pal_types: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AddUpsPalData {
    pub pal_dto: psp_core::dto::pal::PalDto,
    pub source_save_file: Option<String>,
    pub source_player_uid: Option<uuid::Uuid>,
    pub source_player_name: Option<String>,
    pub source_storage_type: Option<String>,
    pub source_storage_slot: Option<i64>,
    pub collection_id: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateUpsPalData {
    pub pal_id: i64,
    pub updates: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
pub struct DeleteUpsPalsData {
    pub pal_ids: Vec<i64>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CloneUpsPalData {
    pub pal_id: i64,
}

/// UPS errors go out as `{"message": "..."}`, NOT the dispatcher's default
/// `{message, trace}` shape.
pub(crate) fn emit_ups_error(ctx: &HandlerCtx<'_>, message: String) {
    ctx.emitter.emit(
        MessageType::Error,
        &serde_json::json!({ "message": message }),
    );
}

fn pals_game_data(ctx: &HandlerCtx<'_>) -> serde_json::Value {
    ctx.app
        .game_data
        .get("pals")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
}

/// element_types -> character ids whose pals.json entry shares any element.
fn character_ids_with_elements(
    pals_data: &serde_json::Value,
    element_types: &[String],
) -> Vec<String> {
    let Some(entries) = pals_data.as_object() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|(_, info)| {
            info.get("element_types")
                .and_then(|v| v.as_array())
                .map(|elements| {
                    elements
                        .iter()
                        .filter_map(|e| e.as_str())
                        .any(|e| element_types.iter().any(|wanted| wanted == e))
                })
                .unwrap_or(false)
        })
        .map(|(character_id, _)| character_id.clone())
        .collect()
}

/// Character ids whose pals.json entry has `is_pal == false`.
fn human_character_ids(pals_data: &serde_json::Value) -> Vec<String> {
    let Some(entries) = pals_data.as_object() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|(_, info)| !info.get("is_pal").and_then(|v| v.as_bool()).unwrap_or(true))
        .map(|(character_id, _)| character_id.clone())
        .collect()
}

fn build_filter(
    pals_data: &serde_json::Value,
    search_query: Option<String>,
    character_id_filter: Option<String>,
    collection_id: Option<i64>,
    tags: Option<Vec<String>>,
    element_types: Option<Vec<String>>,
    pal_types: Option<Vec<String>>,
) -> UpsFilter {
    let element_character_ids = element_types
        .filter(|e| !e.is_empty())
        .map(|e| character_ids_with_elements(pals_data, &e))
        .filter(|ids| !ids.is_empty());
    let pal_type_filters = pal_types.filter(|p| !p.is_empty()).map(|types| {
        types
            .iter()
            .filter_map(|pal_type| match pal_type.as_str() {
                "alpha" => Some(PalTypeFilter::Alpha),
                "lucky" => Some(PalTypeFilter::Lucky),
                "human" => Some(PalTypeFilter::Human(human_character_ids(pals_data))),
                "predator" => Some(PalTypeFilter::Predator),
                "oilrig" => Some(PalTypeFilter::Oilrig),
                "summon" => Some(PalTypeFilter::Summon),
                _ => None,
            })
            .collect()
    });
    UpsFilter {
        search_query,
        character_id_filter,
        collection_id,
        tags,
        element_character_ids,
        pal_types: pal_type_filters,
    }
}

/// pals.json keys, used by `format_character_key` to decide whether a
/// `BOSS_`-prefixed character_id is itself a known key.
fn known_pal_keys(pals_data: &serde_json::Value) -> std::collections::HashSet<String> {
    pals_data
        .as_object()
        .map(|entries| entries.keys().cloned().collect())
        .unwrap_or_default()
}

/// The full 20-key wire object for a UPS pal, including the derived
/// `character_key` the frontend uses to pick an icon.
fn pal_wire_object(
    record: &psp_db::ups::UpsPalRecord,
    known_keys: &std::collections::HashSet<String>,
) -> serde_json::Value {
    serde_json::json!({
        "id": record.id,
        "instance_id": record.instance_id,
        "character_id": record.character_id,
        "character_key": psp_core::dto::pal::format_character_key(&record.character_id, known_keys),
        "nickname": record.nickname,
        "level": record.level,
        "pal_data": record.pal_data,
        "source_save_file": record.source_save_file,
        "source_player_uid": record.source_player_uid,
        "source_player_name": record.source_player_name,
        "source_storage_type": record.source_storage_type,
        "source_storage_slot": record.source_storage_slot,
        "collection_id": record.collection_id,
        "tags": record.tags,
        "notes": record.notes,
        "created_at": record.created_at,
        "updated_at": record.updated_at,
        "last_accessed_at": record.last_accessed_at,
        "transfer_count": record.transfer_count,
        "clone_count": record.clone_count,
    })
}

pub async fn handle_get_ups_pals(
    data: GetUpsPalsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let pals_data = pals_game_data(ctx);
    let filter = build_filter(
        &pals_data,
        data.search_query,
        data.character_id_filter,
        data.collection_id,
        data.tags,
        data.element_types,
        data.pal_types,
    );
    match psp_db::ups::get_pals(
        &ctx.app.db,
        &filter,
        &data.sort_by,
        &data.sort_order,
        data.offset,
        data.limit,
    )
    .await
    {
        Ok((records, total_count)) => {
            let known_keys = known_pal_keys(&pals_data);
            let pal_list: Vec<serde_json::Value> = records
                .iter()
                .map(|r| pal_wire_object(r, &known_keys))
                .collect();
            ctx.emitter.emit(
                MessageType::GetUpsPals,
                &serde_json::json!({
                    "pals": pal_list,
                    "total_count": total_count,
                    "offset": data.offset,
                    "limit": data.limit,
                }),
            );
        }
        Err(error) => emit_ups_error(ctx, format!("Failed to get UPS pals: {error}")),
    }
    Ok(())
}

pub async fn handle_get_ups_all_filtered_ids(
    data: GetUpsAllFilteredIdsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let pals_data = pals_game_data(ctx);
    let filter = build_filter(
        &pals_data,
        data.search_query,
        data.character_id_filter,
        data.collection_id,
        data.tags,
        data.element_types,
        data.pal_types,
    );
    match psp_db::ups::get_all_filtered_ids(&ctx.app.db, &filter).await {
        Ok(pal_ids) => ctx.emitter.emit(
            MessageType::GetUpsAllFilteredIds,
            &serde_json::json!({"pal_ids": pal_ids, "total_count": pal_ids.len()}),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to get filtered UPS pal IDs: {error}")),
    }
    Ok(())
}

pub(crate) fn new_ups_pal_from_dto(
    data: AddUpsPalData,
) -> Result<psp_db::ups::NewUpsPal, HandlerError> {
    let pal_data = serde_json::to_value(&data.pal_dto)?;
    Ok(psp_db::ups::NewUpsPal {
        character_id: data.pal_dto.character_id,
        nickname: data.pal_dto.nickname,
        level: data.pal_dto.level,
        pal_data,
        source_save_file: data.source_save_file,
        source_player_uid: data.source_player_uid.map(|u| u.to_string()),
        source_player_name: data.source_player_name,
        source_storage_type: data.source_storage_type,
        source_storage_slot: data.source_storage_slot,
        collection_id: data.collection_id,
        tags: data.tags.unwrap_or_default(),
        notes: data.notes,
    })
}

pub async fn handle_add_ups_pal(
    data: AddUpsPalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let pals_data = pals_game_data(ctx);
    let new_pal = new_ups_pal_from_dto(data)?;
    match psp_db::ups::add_pal(&ctx.app.db, new_pal, &pals_data).await {
        // The whole record goes out, WITHOUT the derived `character_key` that
        // `get_ups_pals` adds.
        Ok(record) => ctx.emitter.emit(MessageType::AddUpsPal, &record),
        Err(error) => emit_ups_error(ctx, format!("Failed to add UPS pal: {error}")),
    }
    Ok(())
}

pub async fn handle_update_ups_pal(
    data: UpdateUpsPalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match psp_db::ups::update_pal(&ctx.app.db, data.pal_id, &data.updates).await {
        Ok(Some(record)) => ctx.emitter.emit(
            MessageType::UpdateUpsPal,
            &serde_json::json!({"pal": {
                "id": record.id,
                "instance_id": record.instance_id,
                "character_id": record.character_id,
                "nickname": record.nickname,
                "level": record.level,
                "collection_id": record.collection_id,
                "tags": record.tags,
                "notes": record.notes,
                "updated_at": record.updated_at,
            }}),
        ),
        Ok(None) => emit_ups_error(ctx, format!("UPS Pal with ID {} not found", data.pal_id)),
        Err(error) => emit_ups_error(ctx, format!("Failed to update UPS pal: {error}")),
    }
    Ok(())
}

pub async fn handle_delete_ups_pals(
    data: DeleteUpsPalsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let pals_data = pals_game_data(ctx);
    match psp_db::ups::delete_pals(&ctx.app.db, &data.pal_ids, &pals_data).await {
        Ok(deleted_count) => ctx.emitter.emit(
            MessageType::DeleteUpsPals,
            &serde_json::json!({
                "deleted_count": deleted_count,
                "requested_count": data.pal_ids.len(),
            }),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to delete UPS pals: {error}")),
    }
    Ok(())
}

pub async fn handle_clone_ups_pal(
    data: CloneUpsPalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let pals_data = pals_game_data(ctx);
    match psp_db::ups::clone_pal(&ctx.app.db, data.pal_id, &pals_data).await {
        Ok(Some(clone)) => ctx.emitter.emit(
            MessageType::CloneUpsPal,
            &serde_json::json!({
                "original_pal_id": data.pal_id,
                "cloned_pal": {
                    "id": clone.id,
                    "instance_id": clone.instance_id,
                    "character_id": clone.character_id,
                    "nickname": clone.nickname,
                    "level": clone.level,
                    "collection_id": clone.collection_id,
                    "tags": clone.tags,
                    "notes": clone.notes,
                },
            }),
        ),
        Ok(None) => emit_ups_error(ctx, format!("UPS Pal with ID {} not found", data.pal_id)),
        Err(error) => emit_ups_error(ctx, format!("Failed to clone UPS pal: {error}")),
    }
    Ok(())
}

pub async fn handle_get_ups_stats(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let pals_data = pals_game_data(ctx);
    match psp_db::ups::get_stats(&ctx.app.db, &pals_data).await {
        Ok(stats) => ctx.emitter.emit(
            MessageType::GetUpsStats,
            &serde_json::json!({ "stats": stats }),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to get UPS stats: {error}")),
    }
    Ok(())
}

pub async fn handle_nuke_ups_pals(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let pals_data = pals_game_data(ctx);
    let (_, total_count) = match psp_db::ups::get_pals(
        &ctx.app.db,
        &UpsFilter::default(),
        "created_at",
        "desc",
        0,
        1,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            emit_ups_error(ctx, format!("Failed to nuke UPS pals: {error}"));
            return Ok(());
        }
    };
    if total_count == 0 {
        ctx.emitter.emit(
            MessageType::NukeUpsPals,
            &serde_json::json!({"success": true, "deleted_count": 0, "message": "UPS is already empty"}),
        );
        return Ok(());
    }
    match psp_db::ups::nuke_all_pals(&ctx.app.db, &pals_data).await {
        Ok(deleted_count) => ctx.emitter.emit(
            MessageType::NukeUpsPals,
            &serde_json::json!({
                "success": true,
                "deleted_count": deleted_count,
                "message": format!("Successfully deleted {deleted_count} pals from UPS"),
            }),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to nuke UPS pals: {error}")),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateUpsCollectionData {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateUpsCollectionData {
    pub collection_id: i64,
    pub updates: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
pub struct DeleteUpsCollectionData {
    pub collection_id: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateUpsTagData {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateUpsTagData {
    pub tag_id: i64,
    pub updates: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
pub struct DeleteUpsTagData {
    pub tag_id: i64,
}

pub async fn handle_get_ups_collections(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    match psp_db::ups::get_collections(&ctx.app.db).await {
        Ok(collections) => ctx.emitter.emit(
            MessageType::GetUpsCollections,
            &serde_json::json!({ "collections": collections }),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to get UPS collections: {error}")),
    }
    Ok(())
}

pub async fn handle_create_ups_collection(
    data: CreateUpsCollectionData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match psp_db::ups::create_collection(
        &ctx.app.db,
        &data.name,
        data.description.as_deref(),
        data.color.as_deref(),
    )
    .await
    {
        Ok(collection) => ctx.emitter.emit(
            MessageType::CreateUpsCollection,
            &serde_json::json!({ "collection": collection }),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to create UPS collection: {error}")),
    }
    Ok(())
}

pub async fn handle_update_ups_collection(
    data: UpdateUpsCollectionData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match psp_db::ups::update_collection(&ctx.app.db, data.collection_id, &data.updates).await {
        Ok(Some(collection)) => ctx.emitter.emit(
            MessageType::UpdateUpsCollection,
            &serde_json::json!({ "collection": collection }),
        ),
        Ok(None) => emit_ups_error(
            ctx,
            format!("Collection with ID {} not found", data.collection_id),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to update UPS collection: {error}")),
    }
    Ok(())
}

pub async fn handle_delete_ups_collection(
    data: DeleteUpsCollectionData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match psp_db::ups::delete_collection(&ctx.app.db, data.collection_id).await {
        Ok(success) => ctx.emitter.emit(
            MessageType::DeleteUpsCollection,
            &serde_json::json!({"success": success, "collection_id": data.collection_id}),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to delete UPS collection: {error}")),
    }
    Ok(())
}

pub async fn handle_get_ups_tags(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    match psp_db::ups::get_available_tags(&ctx.app.db).await {
        Ok(tags) => ctx.emitter.emit(
            MessageType::GetUpsTags,
            &serde_json::json!({ "tags": tags }),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to get UPS tags: {error}")),
    }
    Ok(())
}

pub async fn handle_create_ups_tag(
    data: CreateUpsTagData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match psp_db::ups::create_or_update_tag(
        &ctx.app.db,
        &data.name,
        data.description.as_deref(),
        data.color.as_deref(),
    )
    .await
    {
        Ok(tag) => ctx.emitter.emit(
            MessageType::CreateUpsTag,
            &serde_json::json!({ "tag": tag }),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to create UPS tag: {error}")),
    }
    Ok(())
}

pub async fn handle_update_ups_tag(
    data: UpdateUpsTagData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match psp_db::ups::update_tag(&ctx.app.db, data.tag_id, &data.updates).await {
        Ok(Some(tag)) => ctx.emitter.emit(
            MessageType::UpdateUpsTag,
            &serde_json::json!({ "tag": tag }),
        ),
        Ok(None) => emit_ups_error(ctx, format!("Tag with ID {} not found", data.tag_id)),
        Err(error) => emit_ups_error(ctx, format!("Failed to update UPS tag: {error}")),
    }
    Ok(())
}

pub async fn handle_delete_ups_tag(
    data: DeleteUpsTagData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match psp_db::ups::delete_tag(&ctx.app.db, data.tag_id).await {
        Ok(success) => ctx.emitter.emit(
            MessageType::DeleteUpsTag,
            &serde_json::json!({"success": success, "tag_id": data.tag_id}),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to delete UPS tag: {error}")),
    }
    Ok(())
}

/// `pal_ids` are instance-id STRINGS, not integers.
#[derive(Debug, serde::Deserialize)]
pub struct CloneToUpsData {
    pub pal_ids: Vec<String>,
    pub source_type: String,
    pub source_player_uid: Option<String>,
    pub collection_id: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ImportToUpsData {
    pub source_type: String,
    pub source_pal_id: Option<uuid::Uuid>,
    pub source_slot: Option<i32>,
    pub source_player_uid: Option<uuid::Uuid>,
    pub collection_id: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ExportUpsPalData {
    pub pal_id: i64,
    pub destination_type: String,
    pub destination_player_uid: Option<uuid::Uuid>,
    pub destination_slot: Option<i32>,
}

/// A source pal resolved out of the loaded save, plus the provenance metadata
/// recorded alongside it in the UPS.
struct ResolvedSourcePal {
    dto: PalDto,
    player_uid: Option<uuid::Uuid>,
    player_name: Option<String>,
    storage_type: String,
    storage_slot: Option<i64>,
}

/// The specific lookup failure `resolve_source_pal` hit. `clone_to_ups` and
/// `import_to_ups` render DIFFERENT user-facing strings for the same failure,
/// so the reason has to survive back to the caller.
enum ResolveError {
    PlayerNotFound,
    PlayerHasNoPals,
    PalNotFoundPalBox,
    GpsNotAvailable,
    PalNotFoundGps,
    PlayerOrDpsNotFound,
    PalNotFoundDps,
    UnknownSourceType,
}

/// Shared source-pal resolution for clone_to_ups/import_to_ups. `gps` and `dps`
/// resolve by SLOT when `source_slot` is `Some` (the import path) and by
/// INSTANCE ID otherwise (the clone path).
fn resolve_source_pal(
    session: &SaveSession,
    game_data: &psp_core::gamedata::GameData,
    source_type: &str,
    pal_instance_id: Option<uuid::Uuid>,
    source_slot: Option<i32>,
    source_player_uid: Option<uuid::Uuid>,
) -> Result<ResolvedSourcePal, ResolveError> {
    match source_type {
        "pal_box" => {
            let player_uid = source_player_uid.ok_or(ResolveError::PlayerNotFound)?;
            let player = build_player_dto(session, game_data, player_uid)
                .map_err(|_| ResolveError::PlayerNotFound)?
                .ok_or(ResolveError::PlayerNotFound)?;
            if player.pals.is_empty() {
                return Err(ResolveError::PlayerHasNoPals);
            }
            let pal_id = pal_instance_id.ok_or(ResolveError::PalNotFoundPalBox)?;
            let dto = player
                .pals
                .get(&pal_id)
                .ok_or(ResolveError::PalNotFoundPalBox)?
                .clone();
            Ok(ResolvedSourcePal {
                dto,
                player_uid: Some(player_uid),
                player_name: Some(player.nickname.clone()),
                storage_type: "pal_box".to_string(),
                storage_slot: None,
            })
        }
        "gps" => {
            let gps_pals = session
                .gps_pals()
                .filter(|pals| !pals.is_empty())
                .ok_or(ResolveError::GpsNotAvailable)?;
            let (slot, dto) = match source_slot {
                Some(slot) => (
                    slot,
                    gps_pals
                        .get(&slot)
                        .ok_or(ResolveError::PalNotFoundGps)?
                        .clone(),
                ),
                None => {
                    let pal_id = pal_instance_id.ok_or(ResolveError::PalNotFoundGps)?;
                    gps_pals
                        .iter()
                        .find(|(_, pal)| pal.instance_id == pal_id)
                        .map(|(slot, pal)| (*slot, pal.clone()))
                        .ok_or(ResolveError::PalNotFoundGps)?
                }
            };
            Ok(ResolvedSourcePal {
                dto,
                player_uid: None,
                player_name: None,
                storage_type: "gps".to_string(),
                storage_slot: Some(slot as i64),
            })
        }
        "dps" => {
            let player_uid = source_player_uid.ok_or(ResolveError::PlayerOrDpsNotFound)?;
            let player = build_player_dto(session, game_data, player_uid)
                .map_err(|_| ResolveError::PlayerOrDpsNotFound)?
                .ok_or(ResolveError::PlayerOrDpsNotFound)?;
            let dps = player
                .dps
                .as_ref()
                .filter(|slots| !slots.is_empty())
                .ok_or(ResolveError::PlayerOrDpsNotFound)?;
            let (slot, dto) = match source_slot {
                Some(slot) => (
                    slot,
                    dps.get(&slot).ok_or(ResolveError::PalNotFoundDps)?.clone(),
                ),
                None => {
                    let pal_id = pal_instance_id.ok_or(ResolveError::PalNotFoundDps)?;
                    dps.iter()
                        .find(|(_, pal)| pal.instance_id == pal_id)
                        .map(|(slot, pal)| (*slot, pal.clone()))
                        .ok_or(ResolveError::PalNotFoundDps)?
                }
            };
            Ok(ResolvedSourcePal {
                dto,
                player_uid: Some(player_uid),
                player_name: Some(player.nickname.clone()),
                storage_type: "dps".to_string(),
                storage_slot: Some(slot as i64),
            })
        }
        _ => Err(ResolveError::UnknownSourceType),
    }
}

fn new_ups_pal_for_source(
    resolved: ResolvedSourcePal,
    collection_id: Option<i64>,
    tags: Option<Vec<String>>,
    notes: Option<String>,
) -> Result<psp_db::ups::NewUpsPal, HandlerError> {
    new_ups_pal_from_dto(AddUpsPalData {
        pal_dto: resolved.dto,
        // The literal "Unknown", not the world name: `get_ups_pals` echoes this
        // field back, and stored pals are expected to show it.
        source_save_file: Some("Unknown".to_string()),
        source_player_uid: resolved.player_uid,
        source_player_name: resolved.player_name,
        source_storage_type: Some(resolved.storage_type),
        source_storage_slot: resolved.storage_slot,
        collection_id,
        tags,
        notes,
    })
}

pub async fn handle_clone_to_ups(
    data: CloneToUpsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if ctx.session.save.is_none() {
        emit_ups_error(ctx, "No save file loaded".to_string());
        return Ok(());
    }
    let mut cloned_count = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for pal_id_text in &data.pal_ids {
        // pal_box and dps need a player uid; gps does not.
        let source_player_uid = match data.source_type.as_str() {
            "pal_box" | "dps" => {
                let Some(raw) = data.source_player_uid.as_deref() else {
                    errors.push(if data.source_type == "pal_box" {
                        format!("Player UID required for pal box clone: {pal_id_text}")
                    } else {
                        format!("Player UID required for DPS clone: {pal_id_text}")
                    });
                    continue;
                };
                match uuid::Uuid::parse_str(raw) {
                    Ok(uid) => Some(uid),
                    Err(error) => {
                        errors.push(format!("Failed to clone {pal_id_text}: {error}"));
                        continue;
                    }
                }
            }
            _ => None,
        };
        // The clone path resolves gps/dps by INSTANCE ID, hence source_slot=None.
        let pal_instance_id = uuid::Uuid::parse_str(pal_id_text).ok();
        let resolved = match resolve_source_pal(
            ctx.session.save.as_ref().unwrap(),
            &ctx.app.game_data,
            &data.source_type,
            pal_instance_id,
            None,
            source_player_uid,
        ) {
            Ok(resolved) => resolved,
            Err(reason) => {
                errors.push(clone_error_string(
                    &data.source_type,
                    reason,
                    pal_id_text,
                    data.source_player_uid.as_deref(),
                ));
                continue;
            }
        };
        let new_pal = new_ups_pal_for_source(
            resolved,
            data.collection_id,
            data.tags.clone(),
            data.notes.clone(),
        )?;
        let pals_data = pals_game_data(ctx);
        match psp_db::ups::add_pal(&ctx.app.db, new_pal, &pals_data).await {
            Ok(_) => cloned_count += 1,
            Err(error) => errors.push(format!("Failed to clone {pal_id_text}: {error}")),
        }
    }

    ctx.emitter.emit(
        MessageType::CloneToUps,
        &serde_json::json!({
            "success": cloned_count > 0,
            "cloned_count": cloned_count,
            "total_requested": data.pal_ids.len(),
            "errors": errors,
        }),
    );
    Ok(())
}

/// Per-pal error strings collected into `clone_to_ups`'s `errors` array.
fn clone_error_string(
    source_type: &str,
    reason: ResolveError,
    pal_id_text: &str,
    source_player_uid: Option<&str>,
) -> String {
    match (source_type, reason) {
        ("pal_box", ResolveError::PlayerNotFound) => {
            format!("Player not found {}", source_player_uid.unwrap_or(""))
        }
        ("pal_box", ResolveError::PlayerHasNoPals) => "Player has no pals".to_string(),
        ("pal_box", _) => format!("Pal not found in player's pal box: {pal_id_text}"),
        ("gps", ResolveError::GpsNotAvailable) => format!("GPS not available for: {pal_id_text}"),
        ("gps", _) => format!("Pal not found in GPS: {pal_id_text}"),
        ("dps", ResolveError::PlayerOrDpsNotFound) => {
            format!("Player or DPS not found for: {pal_id_text}")
        }
        ("dps", _) => format!("Pal not found in DPS: {pal_id_text}"),
        _ => format!("Failed to clone {pal_id_text}"),
    }
}

pub async fn handle_import_to_ups(
    data: ImportToUpsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if ctx.session.save.is_none() {
        emit_ups_error(ctx, "No save file loaded".to_string());
        return Ok(());
    }
    match data.source_type.as_str() {
        "pal_box" if data.source_pal_id.is_none() || data.source_player_uid.is_none() => {
            emit_ups_error(
                ctx,
                "Pal ID and Player UID required for pal box import".into(),
            );
            return Ok(());
        }
        "gps" if data.source_slot.is_none() => {
            emit_ups_error(ctx, "Slot index required for GPS import".into());
            return Ok(());
        }
        "dps" if data.source_slot.is_none() || data.source_player_uid.is_none() => {
            emit_ups_error(
                ctx,
                "Slot index and Player UID required for DPS import".into(),
            );
            return Ok(());
        }
        _ => {}
    }
    let resolved = match resolve_source_pal(
        ctx.session.save.as_ref().unwrap(),
        &ctx.app.game_data,
        &data.source_type,
        data.source_pal_id,
        data.source_slot,
        data.source_player_uid,
    ) {
        Ok(resolved) => resolved,
        Err(reason) => {
            emit_ups_error(
                ctx,
                import_error_string(&data.source_type, reason).to_string(),
            );
            return Ok(());
        }
    };
    let new_pal = new_ups_pal_for_source(resolved, data.collection_id, data.tags, data.notes)?;
    let pals_data = pals_game_data(ctx);
    match psp_db::ups::add_pal(&ctx.app.db, new_pal, &pals_data).await {
        Ok(record) => ctx.emitter.emit(
            MessageType::ImportToUps,
            &serde_json::json!({"success": true, "pal": {
                "id": record.id,
                "instance_id": record.instance_id,
                "character_id": record.character_id,
                "nickname": record.nickname,
                "level": record.level,
                "collection_id": record.collection_id,
                "tags": record.tags,
                "notes": record.notes,
            }}),
        ),
        Err(error) => emit_ups_error(ctx, format!("Failed to import to UPS: {error}")),
    }
    Ok(())
}

/// `import_to_ups`'s hard-error strings — deliberately distinct from
/// `clone_error_string`'s per-pal wording for the same failures.
fn import_error_string(source_type: &str, reason: ResolveError) -> &'static str {
    match (source_type, reason) {
        ("pal_box", ResolveError::PlayerNotFound | ResolveError::PlayerHasNoPals) => {
            "Player or pals not found"
        }
        ("pal_box", _) => "Pal not found in player's pal box",
        ("gps", _) => "Pal not found in GPS slot",
        ("dps", ResolveError::PlayerOrDpsNotFound) => "Player or DPS not found",
        ("dps", _) => "Pal not found in DPS slot",
        _ => "Failed to retrieve Pal data",
    }
}

pub async fn handle_export_ups_pal(
    data: ExportUpsPalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if ctx.session.save.is_none() {
        emit_ups_error(ctx, "No save file loaded".to_string());
        return Ok(());
    }
    let Some(ups_pal) = psp_db::ups::get_pal_by_id(&ctx.app.db, data.pal_id).await? else {
        emit_ups_error(ctx, format!("UPS Pal with ID {} not found", data.pal_id));
        return Ok(());
    };
    let pal_dto = match PalDto::from_json_lenient(&ups_pal.pal_data) {
        Ok(dto) => dto,
        Err(error) => {
            emit_ups_error(ctx, format!("Failed to export UPS pal: {error}"));
            return Ok(());
        }
    };

    let mut player_name: Option<String> = None;
    let mut exported = false;

    match data.destination_type.as_str() {
        "pal_box" => {
            let Some(player_uid) = data.destination_player_uid else {
                emit_ups_error(ctx, "Player UID required for pal box export".into());
                return Ok(());
            };
            // Players are loaded lazily, so `build_player_dto` alone would
            // reject a real-but-not-yet-opened destination player. Check
            // existence against the eagerly built `player_summaries` first, then
            // force-load the player's GVAS. Same guard as
            // `handlers::gps::handle_clone_gps_pal_to_player`.
            let save = ctx.session.save.as_ref().unwrap();
            if !save.player_summaries.contains_key(&player_uid) {
                emit_ups_error(ctx, "Player not found".into());
                return Ok(());
            }
            let save = ctx.session.save_mut()?;
            save.ensure_player_loaded(player_uid)?;
            let save = ctx.session.save.as_ref().unwrap();
            let Some(player) = build_player_dto(save, &ctx.app.game_data, player_uid)? else {
                emit_ups_error(ctx, "Player not found".into());
                return Ok(());
            };
            let Some(pal_box_id) = player.pal_box_id else {
                emit_ups_error(ctx, "Player not found".into());
                return Ok(());
            };
            player_name = Some(player.nickname.clone());
            let save = ctx.session.save_mut()?;
            if let Some(new_pal) = pal::add_player_pal_from_dto(
                save,
                &ctx.app.game_data,
                player_uid,
                &pal_dto,
                pal_box_id,
                None,
            )? {
                exported = true;
                ctx.emitter.emit(
                    MessageType::AddPal,
                    &serde_json::json!({"player_id": player_uid.to_string(), "pal": new_pal}),
                );
            }
        }
        "gps" => {
            let save = ctx.session.save_mut()?;
            if let Some((slot_index, new_pal)) =
                save.add_gps_pal_from_dto(&ctx.app.game_data, &pal_dto, data.destination_slot)?
            {
                exported = true;
                ctx.emitter.emit(
                    MessageType::AddGpsPal,
                    &serde_json::json!({"pal": new_pal, "index": slot_index}),
                );
            }
        }
        "dps" => {
            let Some(player_uid) = data.destination_player_uid else {
                emit_ups_error(ctx, "Player UID required for DPS export".into());
                return Ok(());
            };
            let save = ctx.session.save_mut()?;
            if let Some((slot_index, new_pal)) = pal::add_player_dps_pal_from_dto(
                save,
                &ctx.app.game_data,
                player_uid,
                &pal_dto,
                data.destination_slot,
            )? {
                exported = true;
                ctx.emitter.emit(
                    MessageType::AddDpsPal,
                    &serde_json::json!({
                        "player_id": player_uid.to_string(),
                        "pal": new_pal,
                        "index": slot_index,
                    }),
                );
            }
        }
        _ => {}
    }

    if exported {
        psp_db::ups::export_pal_to_save(
            &ctx.app.db,
            data.pal_id,
            &data.destination_type,
            &psp_db::ups::ExportDestinationInfo {
                // The literal "Unknown", not world_name — see
                // `new_ups_pal_for_source`.
                save_file_name: Some("Unknown".to_string()),
                player_name,
                player_uid: data.destination_player_uid.map(|uid| uid.to_string()),
            },
        )
        .await?;
        ctx.emitter.emit(
            MessageType::ExportUpsPal,
            &serde_json::json!({
                "success": true,
                "destination_type": data.destination_type,
                "destination_player_uid": data.destination_player_uid.map(|uid| uid.to_string()),
                "destination_slot": data.destination_slot,
            }),
        );
    } else {
        ctx.emitter.emit(
            MessageType::ExportUpsPal,
            &serde_json::json!({"success": false, "error": "Failed to export pal to destination"}),
        );
    }
    Ok(())
}
