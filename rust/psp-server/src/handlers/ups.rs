//! UPS pal handlers (Task 3C-4): get/get_ids/add/update/delete/clone/stats/
//! nuke, wired to the psp-db UPS layer (Task 3C-1..3). Wire shapes are ported
//! from `ws/ups_handler.py`.

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
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

/// Python UPS handlers emit error data as {"message": "..."} (ups_handler.py:63-67) —
/// NOT the dispatcher's default {message, trace} shape.
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

/// element_types -> character ids whose pals.json entry shares any element (ups.py:141-160).
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

/// is_pal == false in pals.json (ups.py:177-184).
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
/// `BOSS_`-prefixed character_id is itself a known key (psp_core::dto::pal::
/// format_character_key, a reviewed Phase 2 port of game/utils.py:7-19).
fn known_pal_keys(pals_data: &serde_json::Value) -> std::collections::HashSet<String> {
    pals_data
        .as_object()
        .map(|entries| entries.keys().cloned().collect())
        .unwrap_or_default()
}

/// The 20-key wire dict from ups_handler.py:88-113.
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

/// `data` already groups every field `NewUpsPal` needs (one call site, from
/// `handle_add_ups_pal`) — taking it whole instead of 9 separate parameters
/// keeps this under clippy's too-many-arguments threshold without losing
/// any of the wire-shape documentation the individual fields carried.
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
        // Python responds with the whole model object (ups_handler.py:149) — no
        // character_key, unlike get_ups_pals's 20-key wire dict.
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
