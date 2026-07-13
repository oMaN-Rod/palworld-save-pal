use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::error::DbError;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct UpsPalRecord {
    pub id: i64,
    pub instance_id: String,
    pub character_id: String,
    pub nickname: Option<String>,
    pub level: i64,
    #[sqlx(json)]
    pub pal_data: serde_json::Value,
    pub source_save_file: Option<String>,
    pub source_player_uid: Option<String>,
    pub source_player_name: Option<String>,
    pub source_storage_type: Option<String>,
    pub source_storage_slot: Option<i64>,
    pub collection_id: Option<i64>,
    #[sqlx(json)]
    pub tags: serde_json::Value,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_accessed_at: Option<String>,
    pub transfer_count: i64,
    pub clone_count: i64,
}

#[derive(Debug, Clone, Default)]
pub struct UpsFilter {
    pub search_query: Option<String>,
    pub character_id_filter: Option<String>,
    pub collection_id: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub element_character_ids: Option<Vec<String>>,
    pub pal_types: Option<Vec<PalTypeFilter>>,
}

#[derive(Debug, Clone)]
pub enum PalTypeFilter {
    Alpha,
    Lucky,
    Human(Vec<String>),
    Predator,
    Oilrig,
    Summon,
}

const SORTABLE_COLUMNS: [&str; 19] = [
    "id",
    "instance_id",
    "character_id",
    "nickname",
    "level",
    "pal_data",
    "source_save_file",
    "source_player_uid",
    "source_player_name",
    "source_storage_type",
    "source_storage_slot",
    "collection_id",
    "tags",
    "notes",
    "created_at",
    "updated_at",
    "last_accessed_at",
    "transfer_count",
    "clone_count",
];

struct ConditionWriter {
    any: bool,
}

impl ConditionWriter {
    fn new() -> Self {
        Self { any: false }
    }
    fn next(&mut self, builder: &mut QueryBuilder<'_, Sqlite>) {
        if self.any {
            builder.push(" AND ");
        } else {
            builder.push(" WHERE ");
            self.any = true;
        }
    }
}

fn push_filter(builder: &mut QueryBuilder<'_, Sqlite>, filter: &UpsFilter) {
    let mut writer = ConditionWriter::new();

    if let Some(query) = filter.search_query.as_deref().filter(|q| !q.is_empty()) {
        let pattern = format!("%{}%", query.to_lowercase());
        writer.next(builder);
        builder.push("(lower(character_id) LIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR lower(nickname) LIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR lower(notes) LIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }

    if let Some(character_id) = filter
        .character_id_filter
        .as_deref()
        .filter(|c| !c.is_empty() && *c != "All")
    {
        writer.next(builder);
        builder.push("character_id = ");
        builder.push_bind(character_id.to_string());
    }

    if let Some(collection_id) = filter.collection_id {
        writer.next(builder);
        builder.push("collection_id = ");
        builder.push_bind(collection_id);
    }

    if let Some(tags) = filter.tags.as_ref().filter(|t| !t.is_empty()) {
        for tag in tags {
            let encoded = serde_json::to_string(tag).expect("tag encodes");
            writer.next(builder);
            builder.push("tags LIKE ");
            builder.push_bind(format!("%{encoded}%"));
        }
    }

    if let Some(character_ids) = filter
        .element_character_ids
        .as_ref()
        .filter(|ids| !ids.is_empty())
    {
        writer.next(builder);
        builder.push("character_id IN (");
        let mut separated = builder.separated(", ");
        for character_id in character_ids {
            separated.push_bind(character_id.clone());
        }
        builder.push(")");
    }

    if let Some(pal_types) = filter.pal_types.as_ref().filter(|t| !t.is_empty()) {
        // A Human filter with no ids matches nothing, so it is dropped from the OR group;
        // if that leaves the group empty, no condition is emitted at all (an empty `()`
        // is a syntax error, and `WHERE (false)` would wrongly exclude everything).
        let contributes = pal_types.iter().any(|pt| match pt {
            PalTypeFilter::Human(ids) => !ids.is_empty(),
            _ => true,
        });
        if contributes {
            writer.next(builder);
            builder.push("(");
            let mut first = true;
            for pal_type in pal_types {
                if let PalTypeFilter::Human(ids) = pal_type {
                    if ids.is_empty() {
                        continue;
                    }
                }
                if !first {
                    builder.push(" OR ");
                }
                first = false;
                match pal_type {
                    PalTypeFilter::Alpha => {
                        builder.push("pal_data LIKE '%\"is_boss\":true%'");
                    }
                    PalTypeFilter::Lucky => {
                        builder.push("pal_data LIKE '%\"is_lucky\":true%'");
                    }
                    PalTypeFilter::Human(ids) => {
                        builder.push("character_id IN (");
                        let mut separated = builder.separated(", ");
                        for character_id in ids {
                            separated.push_bind(character_id.clone());
                        }
                        builder.push(")");
                    }
                    PalTypeFilter::Predator => {
                        builder.push("character_id LIKE '%predator_%'");
                    }
                    PalTypeFilter::Oilrig => {
                        builder.push("character_id LIKE '%_oilrig%'");
                    }
                    PalTypeFilter::Summon => {
                        builder.push("character_id LIKE '%summon_%'");
                    }
                }
            }
            builder.push(")");
        }
    }
}

fn sort_clause(sort_by: &str, sort_order: &str) -> String {
    let column = if SORTABLE_COLUMNS.contains(&sort_by) {
        sort_by
    } else {
        "created_at"
    };
    let direction = if sort_order == "desc" { "DESC" } else { "ASC" };
    format!(" ORDER BY {column} {direction}")
}

pub async fn get_pals(
    pool: &SqlitePool,
    filter: &UpsFilter,
    sort_by: &str,
    sort_order: &str,
    offset: i64,
    limit: i64,
) -> Result<(Vec<UpsPalRecord>, i64), DbError> {
    let mut count_builder = QueryBuilder::new("SELECT COUNT(*) FROM ups_pals");
    push_filter(&mut count_builder, filter);
    let total_count: i64 = count_builder.build_query_scalar().fetch_one(pool).await?;

    let mut builder = QueryBuilder::new("SELECT * FROM ups_pals");
    push_filter(&mut builder, filter);
    builder.push(sort_clause(sort_by, sort_order));
    builder.push(" LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);
    let pals = builder
        .build_query_as::<UpsPalRecord>()
        .fetch_all(pool)
        .await?;
    Ok((pals, total_count))
}

pub async fn get_all_filtered_ids(
    pool: &SqlitePool,
    filter: &UpsFilter,
) -> Result<Vec<i64>, DbError> {
    let mut builder = QueryBuilder::new("SELECT id FROM ups_pals");
    push_filter(&mut builder, filter);
    let ids: Vec<i64> = builder.build_query_scalar().fetch_all(pool).await?;
    Ok(ids)
}

pub async fn get_pal_by_id(
    pool: &SqlitePool,
    pal_id: i64,
) -> Result<Option<UpsPalRecord>, DbError> {
    let record = sqlx::query_as::<_, UpsPalRecord>("SELECT * FROM ups_pals WHERE id = ?")
        .bind(pal_id)
        .fetch_optional(pool)
        .await?;
    Ok(record)
}

#[derive(Debug, Clone)]
pub struct NewUpsPal {
    pub character_id: String,
    pub nickname: Option<String>,
    pub level: i64,
    pub pal_data: serde_json::Value,
    pub source_save_file: Option<String>,
    pub source_player_uid: Option<String>,
    pub source_player_name: Option<String>,
    pub source_storage_type: Option<String>,
    pub source_storage_slot: Option<i64>,
    pub collection_id: Option<i64>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TransferLogEntry<'a> {
    pub pal_id: i64,
    pub operation_type: &'a str,
    pub source_type: Option<&'a str>,
    pub destination_type: Option<&'a str>,
    pub save_file_name: Option<&'a str>,
    pub player_name: Option<&'a str>,
    pub player_uid: Option<&'a str>,
    pub success: bool,
}

pub async fn log_transfer(pool: &SqlitePool, entry: TransferLogEntry<'_>) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO ups_transfer_log
         (pal_id, operation_type, source_type, destination_type, save_file_name,
          player_name, player_uid, success, timestamp)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(entry.pal_id)
    .bind(entry.operation_type)
    .bind(entry.source_type)
    .bind(entry.destination_type)
    .bind(entry.save_file_name)
    .bind(entry.player_name)
    .bind(entry.player_uid)
    .bind(entry.success)
    .bind(crate::time::now_iso_naive_utc())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_pal(
    pool: &SqlitePool,
    new_pal: NewUpsPal,
    pals_game_data: &serde_json::Value,
) -> Result<UpsPalRecord, DbError> {
    let now = crate::time::now_iso_naive_utc();
    let pal_id: i64 = sqlx::query_scalar(
        "INSERT INTO ups_pals
         (instance_id, character_id, nickname, level, pal_data, source_save_file,
          source_player_uid, source_player_name, source_storage_type, source_storage_slot,
          collection_id, tags, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&new_pal.character_id)
    .bind(&new_pal.nickname)
    .bind(new_pal.level)
    .bind(new_pal.pal_data.to_string())
    .bind(&new_pal.source_save_file)
    .bind(&new_pal.source_player_uid)
    .bind(&new_pal.source_player_name)
    .bind(&new_pal.source_storage_type)
    .bind(new_pal.source_storage_slot)
    .bind(new_pal.collection_id)
    .bind(serde_json::to_string(&new_pal.tags).expect("tags encode"))
    .bind(&new_pal.notes)
    .bind(&now)
    .bind(&now)
    .fetch_one(pool)
    .await?;

    recompute_stats(pool, pals_game_data).await?;
    update_collection_counts(pool).await?;
    log_transfer(
        pool,
        TransferLogEntry {
            pal_id,
            operation_type: "import",
            source_type: new_pal.source_storage_type.as_deref(),
            destination_type: Some("ups"),
            save_file_name: new_pal.source_save_file.as_deref(),
            player_name: new_pal.source_player_name.as_deref(),
            player_uid: new_pal.source_player_uid.as_deref(),
            success: true,
        },
    )
    .await?;

    Ok(get_pal_by_id(pool, pal_id)
        .await?
        .expect("row just inserted"))
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct UpsStatsRecord {
    pub total_pals: i64,
    pub total_collections: i64,
    pub total_tags: i64,
    pub total_transfers: i64,
    pub total_clones: i64,
    pub storage_size_mb: f64,
    pub most_transferred_pal_id: Option<i64>,
    pub most_cloned_pal_id: Option<i64>,
    pub most_popular_character_id: Option<String>,
    pub element_distribution: String,
    pub alpha_count: i64,
    pub lucky_count: i64,
    pub human_count: i64,
    pub predator_count: i64,
    pub oilrig_count: i64,
    pub summon_count: i64,
    pub last_updated: String,
}

async fn ensure_stats_row(pool: &SqlitePool) -> Result<(), DbError> {
    sqlx::query("INSERT OR IGNORE INTO ups_stats (id, last_updated) VALUES (1, ?)")
        .bind(crate::time::now_iso_naive_utc())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn recompute_stats(
    pool: &SqlitePool,
    pals_game_data: &serde_json::Value,
) -> Result<(), DbError> {
    ensure_stats_row(pool).await?;

    let total_pals: i64 = sqlx::query_scalar("SELECT COUNT(id) FROM ups_pals")
        .fetch_one(pool)
        .await?;
    let total_collections: i64 = sqlx::query_scalar("SELECT COUNT(id) FROM ups_collections")
        .fetch_one(pool)
        .await?;
    let total_tags: i64 = sqlx::query_scalar("SELECT COUNT(id) FROM ups_tags")
        .fetch_one(pool)
        .await?;
    let total_transfers: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(transfer_count), 0) FROM ups_pals")
            .fetch_one(pool)
            .await?;
    let total_clones: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(clone_count), 0) FROM ups_pals")
            .fetch_one(pool)
            .await?;
    let most_transferred: Option<i64> =
        sqlx::query_scalar("SELECT id FROM ups_pals ORDER BY transfer_count DESC LIMIT 1")
            .fetch_optional(pool)
            .await?;
    let most_cloned: Option<i64> =
        sqlx::query_scalar("SELECT id FROM ups_pals ORDER BY clone_count DESC LIMIT 1")
            .fetch_optional(pool)
            .await?;
    let most_popular: Option<String> = sqlx::query_scalar(
        "SELECT character_id FROM ups_pals GROUP BY character_id
         ORDER BY COUNT(character_id) DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    // CAST to BLOB so LENGTH() returns the UTF-8 byte count; on TEXT it counts characters,
    // which under-reports storage for any multi-byte pal_data.
    let total_bytes: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(LENGTH(CAST(pal_data AS BLOB))), 0) FROM ups_pals")
            .fetch_one(pool)
            .await?;
    let storage_size_mb = total_bytes as f64 / (1024.0 * 1024.0);

    let rows: Vec<(String, String)> = sqlx::query_as("SELECT character_id, pal_data FROM ups_pals")
        .fetch_all(pool)
        .await?;
    let mut element_counts: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let (mut alpha, mut lucky, mut human, mut predator, mut oilrig, mut summon) =
        (0i64, 0i64, 0i64, 0i64, 0i64, 0i64);
    for (character_id, pal_data_text) in rows {
        if let Some(character_info) = pals_game_data.get(&character_id) {
            if let Some(elements) = character_info
                .get("element_types")
                .and_then(|v| v.as_array())
            {
                for element in elements.iter().filter_map(|e| e.as_str()) {
                    let current = element_counts
                        .get(element)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    element_counts.insert(element.to_string(), serde_json::json!(current + 1));
                }
            }
            if !character_info
                .get("is_pal")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
            {
                human += 1;
            }
        }
        if let Ok(pal_data) = serde_json::from_str::<serde_json::Value>(&pal_data_text) {
            if pal_data
                .get("is_boss")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                alpha += 1;
            }
            if pal_data
                .get("is_lucky")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                lucky += 1;
            }
        }
        let lower = character_id.to_lowercase();
        if lower.contains("predator_") {
            predator += 1;
        } else if lower.contains("_oilrig") {
            oilrig += 1;
        } else if lower.contains("summon_") {
            summon += 1;
        }
    }

    // COALESCE keeps the last known most_* ids when the table is empty rather than
    // nulling them out.
    sqlx::query(
        "UPDATE ups_stats SET
           total_pals = ?, total_collections = ?, total_tags = ?, total_transfers = ?,
           total_clones = ?, storage_size_mb = ?,
           most_transferred_pal_id = COALESCE(?, most_transferred_pal_id),
           most_cloned_pal_id = COALESCE(?, most_cloned_pal_id),
           most_popular_character_id = COALESCE(?, most_popular_character_id),
           element_distribution = ?, alpha_count = ?, lucky_count = ?, human_count = ?,
           predator_count = ?, oilrig_count = ?, summon_count = ?, last_updated = ?
         WHERE id = 1",
    )
    .bind(total_pals)
    .bind(total_collections)
    .bind(total_tags)
    .bind(total_transfers)
    .bind(total_clones)
    .bind(storage_size_mb)
    .bind(most_transferred)
    .bind(most_cloned)
    .bind(most_popular)
    .bind(serde_json::Value::Object(element_counts).to_string())
    .bind(alpha)
    .bind(lucky)
    .bind(human)
    .bind(predator)
    .bind(oilrig)
    .bind(summon)
    .bind(crate::time::now_iso_utc_offset())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_stats(
    pool: &SqlitePool,
    pals_game_data: &serde_json::Value,
) -> Result<UpsStatsRecord, DbError> {
    ensure_stats_row(pool).await?;
    recompute_stats(pool, pals_game_data).await?;
    let stats = sqlx::query_as::<_, UpsStatsRecord>(
        "SELECT total_pals, total_collections, total_tags, total_transfers, total_clones,
                storage_size_mb, most_transferred_pal_id, most_cloned_pal_id,
                most_popular_character_id, element_distribution, alpha_count, lucky_count,
                human_count, predator_count, oilrig_count, summon_count, last_updated
         FROM ups_stats WHERE id = 1",
    )
    .fetch_one(pool)
    .await?;
    Ok(stats)
}

pub async fn update_collection_counts(pool: &SqlitePool) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE ups_collections SET
           pal_count = (SELECT COUNT(*) FROM ups_pals WHERE ups_pals.collection_id = ups_collections.id),
           updated_at = ?",
    )
    .bind(crate::time::now_iso_naive_utc())
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct UpsCollectionRecord {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub is_favorite: bool,
    pub is_archived: bool,
    pub pal_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn create_collection(
    pool: &SqlitePool,
    name: &str,
    description: Option<&str>,
    color: Option<&str>,
) -> Result<UpsCollectionRecord, DbError> {
    let now = crate::time::now_iso_naive_utc();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO ups_collections (name, description, color, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(name)
    .bind(description)
    .bind(color)
    .bind(&now)
    .bind(&now)
    .fetch_one(pool)
    .await?;
    Ok(get_collection_by_id(pool, id)
        .await?
        .expect("row just inserted"))
}

pub async fn get_collection_by_id(
    pool: &SqlitePool,
    collection_id: i64,
) -> Result<Option<UpsCollectionRecord>, DbError> {
    let record =
        sqlx::query_as::<_, UpsCollectionRecord>("SELECT * FROM ups_collections WHERE id = ?")
            .bind(collection_id)
            .fetch_optional(pool)
            .await?;
    Ok(record)
}

pub async fn get_collections(pool: &SqlitePool) -> Result<Vec<UpsCollectionRecord>, DbError> {
    let records =
        sqlx::query_as::<_, UpsCollectionRecord>("SELECT * FROM ups_collections ORDER BY name")
            .fetch_all(pool)
            .await?;
    Ok(records)
}

const SYNCED_COLUMNS: [&str; 3] = ["character_id", "nickname", "level"];
const UPDATABLE_COLUMNS: [&str; 16] = [
    "instance_id",
    "character_id",
    "nickname",
    "level",
    "pal_data",
    "source_save_file",
    "source_player_uid",
    "source_player_name",
    "source_storage_type",
    "source_storage_slot",
    "collection_id",
    "tags",
    "notes",
    "last_accessed_at",
    "transfer_count",
    "clone_count",
];

pub async fn update_pal(
    pool: &SqlitePool,
    pal_id: i64,
    updates: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<UpsPalRecord>, DbError> {
    let Some(mut record) = get_pal_by_id(pool, pal_id).await? else {
        return Ok(None);
    };

    for (key, value) in updates {
        if !UPDATABLE_COLUMNS.contains(&key.as_str()) {
            continue;
        }
        match key.as_str() {
            "instance_id" => {
                if let Some(v) = value.as_str() {
                    record.instance_id = v.to_string();
                }
            }
            "character_id" => {
                if let Some(v) = value.as_str() {
                    record.character_id = v.to_string();
                }
            }
            "nickname" => record.nickname = value.as_str().map(str::to_string),
            "level" => {
                if let Some(v) = value.as_i64() {
                    record.level = v;
                }
            }
            "pal_data" => record.pal_data = value.clone(),
            "source_save_file" => record.source_save_file = value.as_str().map(str::to_string),
            "source_player_uid" => record.source_player_uid = value.as_str().map(str::to_string),
            "source_player_name" => record.source_player_name = value.as_str().map(str::to_string),
            "source_storage_type" => {
                record.source_storage_type = value.as_str().map(str::to_string)
            }
            "source_storage_slot" => record.source_storage_slot = value.as_i64(),
            "collection_id" => record.collection_id = value.as_i64(),
            "tags" => {
                if value.is_array() {
                    record.tags = value.clone();
                }
            }
            "notes" => record.notes = value.as_str().map(str::to_string),
            "last_accessed_at" => record.last_accessed_at = value.as_str().map(str::to_string),
            "transfer_count" => {
                if let Some(v) = value.as_i64() {
                    record.transfer_count = v;
                }
            }
            "clone_count" => {
                if let Some(v) = value.as_i64() {
                    record.clone_count = v;
                }
            }
            _ => {}
        }
    }

    // character_id/nickname/level are denormalized out of the pal_data JSON so they can be
    // filtered and sorted on; whichever side the caller updated becomes the source of truth.
    if updates.contains_key("pal_data") {
        if let Some(pal_data) = record.pal_data.as_object() {
            if let Some(v) = pal_data.get("character_id").and_then(|v| v.as_str()) {
                record.character_id = v.to_string();
            }
            if let Some(v) = pal_data.get("nickname") {
                record.nickname = v.as_str().map(str::to_string);
            }
            if let Some(v) = pal_data.get("level").and_then(|v| v.as_i64()) {
                record.level = v;
            }
        }
    } else {
        let updated_synced: Vec<&str> = SYNCED_COLUMNS
            .iter()
            .copied()
            .filter(|c| updates.contains_key(*c))
            .collect();
        if !updated_synced.is_empty() {
            if let Some(pal_data) = record.pal_data.as_object_mut() {
                for column in updated_synced {
                    let new_value = match column {
                        "character_id" => serde_json::json!(record.character_id),
                        "nickname" => serde_json::json!(record.nickname),
                        "level" => serde_json::json!(record.level),
                        _ => unreachable!(),
                    };
                    pal_data.insert(column.to_string(), new_value);
                }
            }
        }
    }

    record.updated_at = crate::time::now_iso_utc_offset();

    sqlx::query(
        "UPDATE ups_pals SET instance_id = ?, character_id = ?, nickname = ?, level = ?,
           pal_data = ?, source_save_file = ?, source_player_uid = ?, source_player_name = ?,
           source_storage_type = ?, source_storage_slot = ?, collection_id = ?, tags = ?,
           notes = ?, updated_at = ?, last_accessed_at = ?, transfer_count = ?, clone_count = ?
         WHERE id = ?",
    )
    .bind(&record.instance_id)
    .bind(&record.character_id)
    .bind(&record.nickname)
    .bind(record.level)
    .bind(record.pal_data.to_string())
    .bind(&record.source_save_file)
    .bind(&record.source_player_uid)
    .bind(&record.source_player_name)
    .bind(&record.source_storage_type)
    .bind(record.source_storage_slot)
    .bind(record.collection_id)
    .bind(record.tags.to_string())
    .bind(&record.notes)
    .bind(&record.updated_at)
    .bind(&record.last_accessed_at)
    .bind(record.transfer_count)
    .bind(record.clone_count)
    .bind(pal_id)
    .execute(pool)
    .await?;

    if updates.contains_key("collection_id") {
        update_collection_counts(pool).await?;
    }
    Ok(Some(record))
}

pub async fn delete_pals(
    pool: &SqlitePool,
    pal_ids: &[i64],
    pals_game_data: &serde_json::Value,
) -> Result<i64, DbError> {
    let mut deleted = 0i64;
    for pal_id in pal_ids {
        if get_pal_by_id(pool, *pal_id).await?.is_some() {
            log_transfer(
                pool,
                TransferLogEntry {
                    pal_id: *pal_id,
                    operation_type: "delete",
                    source_type: Some("ups"),
                    success: true,
                    ..Default::default()
                },
            )
            .await?;
            sqlx::query("DELETE FROM ups_pals WHERE id = ?")
                .bind(pal_id)
                .execute(pool)
                .await?;
            deleted += 1;
        }
    }
    recompute_stats(pool, pals_game_data).await?;
    update_collection_counts(pool).await?;
    Ok(deleted)
}

pub async fn clone_pal(
    pool: &SqlitePool,
    pal_id: i64,
    pals_game_data: &serde_json::Value,
) -> Result<Option<UpsPalRecord>, DbError> {
    let Some(original) = get_pal_by_id(pool, pal_id).await? else {
        return Ok(None);
    };
    let clone_nickname = original.nickname.as_ref().map(|n| format!("{n} (Clone)"));
    let clone_notes = format!(
        "Clone of {}",
        original
            .nickname
            .clone()
            .unwrap_or_else(|| original.character_id.clone())
    );
    let now = crate::time::now_iso_naive_utc();
    let clone_id: i64 = sqlx::query_scalar(
        "INSERT INTO ups_pals
         (instance_id, character_id, nickname, level, pal_data, source_save_file,
          source_player_uid, source_player_name, source_storage_type, collection_id,
          tags, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'ups_clone', ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&original.character_id)
    .bind(&clone_nickname)
    .bind(original.level)
    .bind(original.pal_data.to_string())
    .bind(&original.source_save_file)
    .bind(&original.source_player_uid)
    .bind(&original.source_player_name)
    .bind(original.collection_id)
    .bind(original.tags.to_string())
    .bind(&clone_notes)
    .bind(&now)
    .bind(&now)
    .fetch_one(pool)
    .await?;

    sqlx::query("UPDATE ups_pals SET clone_count = clone_count + 1 WHERE id = ?")
        .bind(pal_id)
        .execute(pool)
        .await?;
    recompute_stats(pool, pals_game_data).await?;
    update_collection_counts(pool).await?;
    log_transfer(
        pool,
        TransferLogEntry {
            pal_id: clone_id,
            operation_type: "clone",
            source_type: Some("ups"),
            destination_type: Some("ups"),
            success: true,
            ..Default::default()
        },
    )
    .await?;
    get_pal_by_id(pool, clone_id).await
}

pub async fn nuke_all_pals(
    pool: &SqlitePool,
    pals_game_data: &serde_json::Value,
) -> Result<i64, DbError> {
    let all_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM ups_pals")
        .fetch_all(pool)
        .await?;
    if all_ids.is_empty() {
        return Ok(0);
    }
    for pal_id in &all_ids {
        log_transfer(
            pool,
            TransferLogEntry {
                pal_id: *pal_id,
                operation_type: "nuke_delete",
                source_type: Some("ups"),
                success: true,
                ..Default::default()
            },
        )
        .await?;
    }
    sqlx::query("DELETE FROM ups_pals").execute(pool).await?;
    sqlx::query("UPDATE ups_collections SET pal_count = 0, updated_at = ?")
        .bind(crate::time::now_iso_naive_utc())
        .execute(pool)
        .await?;
    recompute_stats(pool, pals_game_data).await?;
    Ok(all_ids.len() as i64)
}

#[derive(Debug, Clone, Default)]
pub struct ExportDestinationInfo {
    pub save_file_name: Option<String>,
    pub player_name: Option<String>,
    pub player_uid: Option<String>,
}

pub async fn export_pal_to_save(
    pool: &SqlitePool,
    pal_id: i64,
    destination_type: &str,
    destination: &ExportDestinationInfo,
) -> Result<bool, DbError> {
    if get_pal_by_id(pool, pal_id).await?.is_none() {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE ups_pals SET last_accessed_at = ?, transfer_count = transfer_count + 1 WHERE id = ?",
    )
    .bind(crate::time::now_iso_utc_offset())
    .bind(pal_id)
    .execute(pool)
    .await?;
    log_transfer(
        pool,
        TransferLogEntry {
            pal_id,
            operation_type: "export",
            source_type: Some("ups"),
            destination_type: Some(destination_type),
            save_file_name: destination.save_file_name.as_deref(),
            player_name: destination.player_name.as_deref(),
            player_uid: destination.player_uid.as_deref(),
            success: true,
        },
    )
    .await?;
    Ok(true)
}

pub async fn update_collection(
    pool: &SqlitePool,
    collection_id: i64,
    updates: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<UpsCollectionRecord>, DbError> {
    let Some(mut record) = get_collection_by_id(pool, collection_id).await? else {
        return Ok(None);
    };
    for (key, value) in updates {
        match key.as_str() {
            "name" => {
                if let Some(v) = value.as_str() {
                    record.name = v.to_string();
                }
            }
            "description" => record.description = value.as_str().map(str::to_string),
            "color" => record.color = value.as_str().map(str::to_string),
            "icon" => record.icon = value.as_str().map(str::to_string),
            "is_favorite" => {
                if let Some(v) = value.as_bool() {
                    record.is_favorite = v;
                }
            }
            "is_archived" => {
                if let Some(v) = value.as_bool() {
                    record.is_archived = v;
                }
            }
            "pal_count" => {
                if let Some(v) = value.as_i64() {
                    record.pal_count = v;
                }
            }
            _ => {}
        }
    }
    record.updated_at = crate::time::now_iso_utc_offset();
    sqlx::query(
        "UPDATE ups_collections SET name = ?, description = ?, color = ?, icon = ?,
           is_favorite = ?, is_archived = ?, pal_count = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&record.name)
    .bind(&record.description)
    .bind(&record.color)
    .bind(&record.icon)
    .bind(record.is_favorite)
    .bind(record.is_archived)
    .bind(record.pal_count)
    .bind(&record.updated_at)
    .bind(collection_id)
    .execute(pool)
    .await?;
    Ok(Some(record))
}

pub async fn delete_collection(pool: &SqlitePool, collection_id: i64) -> Result<bool, DbError> {
    if get_collection_by_id(pool, collection_id).await?.is_none() {
        return Ok(false);
    }
    sqlx::query("UPDATE ups_pals SET collection_id = NULL WHERE collection_id = ?")
        .bind(collection_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM ups_collections WHERE id = ?")
        .bind(collection_id)
        .execute(pool)
        .await?;
    Ok(true)
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct UpsTagRecord {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub usage_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn get_tag_by_id(
    pool: &SqlitePool,
    tag_id: i64,
) -> Result<Option<UpsTagRecord>, DbError> {
    let record = sqlx::query_as::<_, UpsTagRecord>("SELECT * FROM ups_tags WHERE id = ?")
        .bind(tag_id)
        .fetch_optional(pool)
        .await?;
    Ok(record)
}

pub async fn get_available_tags(pool: &SqlitePool) -> Result<Vec<UpsTagRecord>, DbError> {
    let records = sqlx::query_as::<_, UpsTagRecord>("SELECT * FROM ups_tags ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(records)
}

pub async fn create_or_update_tag(
    pool: &SqlitePool,
    name: &str,
    description: Option<&str>,
    color: Option<&str>,
) -> Result<UpsTagRecord, DbError> {
    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM ups_tags WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    match existing {
        Some(tag_id) => {
            if let Some(description) = description {
                sqlx::query("UPDATE ups_tags SET description = ? WHERE id = ?")
                    .bind(description)
                    .bind(tag_id)
                    .execute(pool)
                    .await?;
            }
            if let Some(color) = color {
                sqlx::query("UPDATE ups_tags SET color = ? WHERE id = ?")
                    .bind(color)
                    .bind(tag_id)
                    .execute(pool)
                    .await?;
            }
            sqlx::query("UPDATE ups_tags SET updated_at = ? WHERE id = ?")
                .bind(crate::time::now_iso_utc_offset())
                .bind(tag_id)
                .execute(pool)
                .await?;
            Ok(get_tag_by_id(pool, tag_id).await?.expect("existing tag"))
        }
        None => {
            let now = crate::time::now_iso_naive_utc();
            let tag_id: i64 = sqlx::query_scalar(
                "INSERT INTO ups_tags (name, description, color, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(name)
            .bind(description)
            .bind(color)
            .bind(&now)
            .bind(&now)
            .fetch_one(pool)
            .await?;
            Ok(get_tag_by_id(pool, tag_id)
                .await?
                .expect("row just inserted"))
        }
    }
}

async fn rewrite_pal_tags(
    pool: &SqlitePool,
    tag_name: &str,
    replacement: Option<&str>,
) -> Result<(), DbError> {
    let encoded = serde_json::to_string(tag_name).expect("tag encodes");
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, tags FROM ups_pals WHERE tags LIKE ?")
            .bind(format!("%{encoded}%"))
            .fetch_all(pool)
            .await?;
    for (pal_id, tags_text) in rows {
        let Ok(serde_json::Value::Array(tags)) = serde_json::from_str(&tags_text) else {
            continue;
        };
        if !tags.iter().any(|t| t.as_str() == Some(tag_name)) {
            continue;
        }
        let rewritten: Vec<serde_json::Value> = tags
            .into_iter()
            .filter_map(|tag| match tag.as_str() {
                Some(current) if current == tag_name => {
                    replacement.map(|new_name| serde_json::json!(new_name))
                }
                _ => Some(tag),
            })
            .collect();
        sqlx::query("UPDATE ups_pals SET tags = ?, updated_at = ? WHERE id = ?")
            .bind(serde_json::Value::Array(rewritten).to_string())
            .bind(crate::time::now_iso_utc_offset())
            .bind(pal_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn update_tag(
    pool: &SqlitePool,
    tag_id: i64,
    updates: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<UpsTagRecord>, DbError> {
    let Some(mut record) = get_tag_by_id(pool, tag_id).await? else {
        return Ok(None);
    };
    let old_name = record.name.clone();
    for (key, value) in updates {
        match key.as_str() {
            "name" => {
                if let Some(v) = value.as_str() {
                    record.name = v.to_string();
                }
            }
            "description" => record.description = value.as_str().map(str::to_string),
            "color" => record.color = value.as_str().map(str::to_string),
            "usage_count" => {
                if let Some(v) = value.as_i64() {
                    record.usage_count = v;
                }
            }
            _ => {}
        }
    }
    record.updated_at = crate::time::now_iso_utc_offset();
    sqlx::query(
        "UPDATE ups_tags SET name = ?, description = ?, color = ?, usage_count = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&record.name)
    .bind(&record.description)
    .bind(&record.color)
    .bind(record.usage_count)
    .bind(&record.updated_at)
    .bind(tag_id)
    .execute(pool)
    .await?;
    if updates.contains_key("name") && old_name != record.name {
        rewrite_pal_tags(pool, &old_name, Some(&record.name)).await?;
    }
    Ok(Some(record))
}

pub async fn delete_tag(pool: &SqlitePool, tag_id: i64) -> Result<bool, DbError> {
    let Some(record) = get_tag_by_id(pool, tag_id).await? else {
        return Ok(false);
    };
    rewrite_pal_tags(pool, &record.name, None).await?;
    sqlx::query("DELETE FROM ups_tags WHERE id = ?")
        .bind(tag_id)
        .execute(pool)
        .await?;
    Ok(true)
}
