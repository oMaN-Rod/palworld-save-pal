use crate::error::DbError;

#[derive(Debug, Clone, serde::Serialize)]
pub struct UpsPalRecord {
    pub id: i64,
    pub instance_id: String,
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
    pub tags: serde_json::Value,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_accessed_at: Option<String>,
    pub transfer_count: i64,
    pub clone_count: i64,
}

fn map_pal(r: &crate::DbRow) -> Result<UpsPalRecord, DbError> {
    Ok(UpsPalRecord {
        id: r.get_i64("id")?,
        instance_id: r.get_string("instance_id")?,
        character_id: r.get_string("character_id")?,
        nickname: r.get_opt_str("nickname")?,
        level: r.get_i64("level")?,
        pal_data: r.get_json("pal_data")?,
        source_save_file: r.get_opt_str("source_save_file")?,
        source_player_uid: r.get_opt_str("source_player_uid")?,
        source_player_name: r.get_opt_str("source_player_name")?,
        source_storage_type: r.get_opt_str("source_storage_type")?,
        source_storage_slot: r.get_opt_i64("source_storage_slot")?,
        collection_id: r.get_opt_i64("collection_id")?,
        tags: r.get_json("tags")?,
        notes: r.get_opt_str("notes")?,
        created_at: r.get_string("created_at")?,
        updated_at: r.get_string("updated_at")?,
        last_accessed_at: r.get_opt_str("last_accessed_at")?,
        transfer_count: r.get_i64("transfer_count")?,
        clone_count: r.get_i64("clone_count")?,
    })
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
    Awakened,
    Imported,
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
    fn next(&mut self, builder: &mut crate::SqlBuilder) {
        if self.any {
            builder.push(" AND ");
        } else {
            builder.push(" WHERE ");
            self.any = true;
        }
    }
}

fn push_filter(builder: &mut crate::SqlBuilder, filter: &UpsFilter) {
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
                    PalTypeFilter::Awakened => {
                        builder.push("pal_data LIKE '%\"is_awakened\":true%'");
                    }
                    PalTypeFilter::Imported => {
                        builder.push("pal_data LIKE '%\"is_imported\":true%'");
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
    db: &dyn crate::DbDriver,
    filter: &UpsFilter,
    sort_by: &str,
    sort_order: &str,
    offset: i64,
    limit: i64,
) -> Result<(Vec<UpsPalRecord>, i64), DbError> {
    let mut count_builder = crate::SqlBuilder::new("SELECT COUNT(*) FROM ups_pals");
    push_filter(&mut count_builder, filter);
    let (count_sql, count_params) = count_builder.into_parts();
    let total_count = crate::scalar_i64(&db.query(&count_sql, &count_params).await?)?;

    let mut builder = crate::SqlBuilder::new("SELECT * FROM ups_pals");
    push_filter(&mut builder, filter);
    builder.push(&sort_clause(sort_by, sort_order));
    builder.push(" LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);
    let (sql, params) = builder.into_parts();
    let pals = db
        .query(&sql, &params)
        .await?
        .iter()
        .map(map_pal)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((pals, total_count))
}

pub async fn get_all_filtered_ids(
    db: &dyn crate::DbDriver,
    filter: &UpsFilter,
) -> Result<Vec<i64>, DbError> {
    let mut builder = crate::SqlBuilder::new("SELECT id FROM ups_pals");
    push_filter(&mut builder, filter);
    let (sql, params) = builder.into_parts();
    db.query(&sql, &params)
        .await?
        .iter()
        .map(|r| r.get_i64_at(0))
        .collect()
}

pub async fn get_pal_by_id(
    db: &dyn crate::DbDriver,
    pal_id: i64,
) -> Result<Option<UpsPalRecord>, DbError> {
    let rows = db
        .query("SELECT * FROM ups_pals WHERE id = ?", &[pal_id.into()])
        .await?;
    rows.first().map(map_pal).transpose()
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

pub async fn log_transfer(
    db: &dyn crate::DbDriver,
    entry: TransferLogEntry<'_>,
) -> Result<(), DbError> {
    db.execute(
        "INSERT INTO ups_transfer_log
         (pal_id, operation_type, source_type, destination_type, save_file_name,
          player_name, player_uid, success, timestamp)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            entry.pal_id.into(),
            entry.operation_type.into(),
            entry.source_type.into(),
            entry.destination_type.into(),
            entry.save_file_name.into(),
            entry.player_name.into(),
            entry.player_uid.into(),
            entry.success.into(),
            crate::time::now_iso_naive_utc().into(),
        ],
    )
    .await?;
    Ok(())
}

/// Chunk size for IN lists and multi-row VALUES: keeps the bound-parameter
/// count per statement well under SQLite's variable limit on every driver
/// (native sqlx and the wasm OPFS bridge alike).
const SQL_CHUNK: usize = 500;

/// Column 0 of every row as an i64.
fn ids_from_rows(rows: &[crate::DbRow]) -> Result<Vec<i64>, DbError> {
    rows.iter().map(|r| r.get_i64_at(0)).collect()
}

/// Multi-row sibling of `log_transfer`'s insert: one statement per chunk,
/// writing exactly the rows `log_transfer` would write for
/// `{source_type: Some("ups"), success: true}` with every other column NULL.
/// Each row gets its own timestamp, as N `log_transfer` calls would.
fn build_ups_log_rows(pal_ids: &[i64], operation_type: &str) -> Vec<(String, Vec<crate::DbValue>)> {
    let mut statements = Vec::new();
    for chunk in pal_ids.chunks(SQL_CHUNK) {
        let mut sql = String::from(
            "INSERT INTO ups_transfer_log
             (pal_id, operation_type, source_type, destination_type, save_file_name,
              player_name, player_uid, success, timestamp)
             VALUES ",
        );
        let mut params: Vec<crate::DbValue> = Vec::with_capacity(chunk.len() * 9);
        for (i, pal_id) in chunk.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            sql.push_str("(?, ?, ?, ?, ?, ?, ?, ?, ?)");
            params.extend([
                (*pal_id).into(),
                operation_type.into(),
                "ups".into(),
                crate::DbValue::Null,
                crate::DbValue::Null,
                crate::DbValue::Null,
                crate::DbValue::Null,
                true.into(),
                crate::time::now_iso_naive_utc().into(),
            ]);
        }
        statements.push((sql, params));
    }
    statements
}

/// One chunked `DELETE ... WHERE id IN (...)` per chunk.
fn build_delete_statements(pal_ids: &[i64]) -> Vec<(String, Vec<crate::DbValue>)> {
    let mut statements = Vec::new();
    for chunk in pal_ids.chunks(SQL_CHUNK) {
        let mut builder = crate::SqlBuilder::new("DELETE FROM ups_pals WHERE id IN (");
        let mut separated = builder.separated(", ");
        for pal_id in chunk {
            separated.push_bind(*pal_id);
        }
        builder.push(")");
        statements.push(builder.into_parts());
    }
    statements
}

/// Adapts owned (sql, params) pairs to the borrowed shape `execute_batch` takes.
fn as_batch(statements: &[(String, Vec<crate::DbValue>)]) -> Vec<(&str, Vec<crate::DbValue>)> {
    statements
        .iter()
        .map(|(sql, params)| (sql.as_str(), params.clone()))
        .collect()
}

pub async fn add_pal(
    db: &dyn crate::DbDriver,
    new_pal: NewUpsPal,
    pals_game_data: &serde_json::Value,
) -> Result<UpsPalRecord, DbError> {
    let now = crate::time::now_iso_naive_utc();
    // RETURNING * yields the full inserted row (column defaults included), so no
    // follow-up SELECT is needed. An empty result must surface as an error, not
    // a panic: on wasm `panic = abort` would kill the module.
    let record = db
        .query(
            "INSERT INTO ups_pals
         (instance_id, character_id, nickname, level, pal_data, source_save_file,
          source_player_uid, source_player_name, source_storage_type, source_storage_slot,
          collection_id, tags, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
            &[
                uuid::Uuid::new_v4().to_string().into(),
                new_pal.character_id.clone().into(),
                new_pal.nickname.clone().into(),
                new_pal.level.into(),
                new_pal.pal_data.to_string().into(),
                new_pal.source_save_file.clone().into(),
                new_pal.source_player_uid.clone().into(),
                new_pal.source_player_name.clone().into(),
                new_pal.source_storage_type.clone().into(),
                new_pal.source_storage_slot.into(),
                new_pal.collection_id.into(),
                serde_json::to_string(&new_pal.tags)
                    .expect("tags encode")
                    .into(),
                new_pal.notes.clone().into(),
                now.clone().into(),
                now.clone().into(),
            ],
        )
        .await?
        .first()
        .map(map_pal)
        .transpose()?
        .ok_or_else(|| DbError::Backend("INSERT RETURNING produced no row".into()))?;

    recompute_stats(db, pals_game_data).await?;
    update_collection_counts(db).await?;
    log_transfer(
        db,
        TransferLogEntry {
            pal_id: record.id,
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

    Ok(record)
}

#[derive(Debug, Clone, serde::Serialize)]
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
    pub awakened_count: i64,
    pub imported_count: i64,
    pub human_count: i64,
    pub predator_count: i64,
    pub oilrig_count: i64,
    pub summon_count: i64,
    pub last_updated: String,
}

fn map_stats(r: &crate::DbRow) -> Result<UpsStatsRecord, DbError> {
    Ok(UpsStatsRecord {
        total_pals: r.get_i64("total_pals")?,
        total_collections: r.get_i64("total_collections")?,
        total_tags: r.get_i64("total_tags")?,
        total_transfers: r.get_i64("total_transfers")?,
        total_clones: r.get_i64("total_clones")?,
        storage_size_mb: r.get_f64("storage_size_mb")?,
        most_transferred_pal_id: r.get_opt_i64("most_transferred_pal_id")?,
        most_cloned_pal_id: r.get_opt_i64("most_cloned_pal_id")?,
        most_popular_character_id: r.get_opt_str("most_popular_character_id")?,
        element_distribution: r.get_string("element_distribution")?,
        alpha_count: r.get_i64("alpha_count")?,
        lucky_count: r.get_i64("lucky_count")?,
        awakened_count: r.get_i64("awakened_count")?,
        imported_count: r.get_i64("imported_count")?,
        human_count: r.get_i64("human_count")?,
        predator_count: r.get_i64("predator_count")?,
        oilrig_count: r.get_i64("oilrig_count")?,
        summon_count: r.get_i64("summon_count")?,
        last_updated: r.get_string("last_updated")?,
    })
}

async fn ensure_stats_row(db: &dyn crate::DbDriver) -> Result<(), DbError> {
    db.execute(
        "INSERT OR IGNORE INTO ups_stats (id, last_updated) VALUES (1, ?)",
        &[crate::time::now_iso_naive_utc().into()],
    )
    .await?;
    Ok(())
}

pub async fn recompute_stats(
    db: &dyn crate::DbDriver,
    pals_game_data: &serde_json::Value,
) -> Result<(), DbError> {
    ensure_stats_row(db).await?;

    // One round-trip for the plain COUNT/SUM stats; each scalar subquery is the
    // former standalone query verbatim.
    let counts = db
        .query(
            "SELECT
               (SELECT COUNT(id) FROM ups_pals),
               (SELECT COUNT(id) FROM ups_collections),
               (SELECT COUNT(id) FROM ups_tags),
               (SELECT COALESCE(SUM(transfer_count), 0) FROM ups_pals),
               (SELECT COALESCE(SUM(clone_count), 0) FROM ups_pals),
               (SELECT COALESCE(SUM(LENGTH(CAST(pal_data AS BLOB))), 0) FROM ups_pals)",
            &[],
        )
        .await?;
    let counts = counts
        .first()
        .ok_or_else(|| DbError::Backend("stats count query returned no rows".into()))?;
    let total_pals = counts.get_i64_at(0)?;
    let total_collections = counts.get_i64_at(1)?;
    let total_tags = counts.get_i64_at(2)?;
    let total_transfers = counts.get_i64_at(3)?;
    let total_clones = counts.get_i64_at(4)?;
    // CAST to BLOB so LENGTH() returns the UTF-8 byte count; on TEXT it counts characters,
    // which under-reports storage for any multi-byte pal_data.
    let total_bytes = counts.get_i64_at(5)?;
    let storage_size_mb = total_bytes as f64 / (1024.0 * 1024.0);

    let most_transferred: Option<i64> = crate::opt_scalar_i64(
        &db.query(
            "SELECT id FROM ups_pals ORDER BY transfer_count DESC LIMIT 1",
            &[],
        )
        .await?,
    )?;
    let most_cloned: Option<i64> = crate::opt_scalar_i64(
        &db.query(
            "SELECT id FROM ups_pals ORDER BY clone_count DESC LIMIT 1",
            &[],
        )
        .await?,
    )?;
    let most_popular: Option<String> = db
        .query(
            "SELECT character_id FROM ups_pals GROUP BY character_id
         ORDER BY COUNT(character_id) DESC LIMIT 1",
            &[],
        )
        .await?
        .first()
        .map(|r| r.get_opt_str_at(0))
        .transpose()?
        .flatten();

    // The pal_data boolean flags counted in SQL, using the same LIKE patterns
    // the Alpha/Lucky/Awakened/Imported filters use, instead of parsing every
    // row's JSON in Rust.
    let flags = db
        .query(
            r#"SELECT
               COALESCE(SUM(pal_data LIKE '%"is_boss":true%'), 0),
               COALESCE(SUM(pal_data LIKE '%"is_lucky":true%'), 0),
               COALESCE(SUM(pal_data LIKE '%"is_awakened":true%'), 0),
               COALESCE(SUM(pal_data LIKE '%"is_imported":true%'), 0)
             FROM ups_pals"#,
            &[],
        )
        .await?;
    let flags = flags
        .first()
        .ok_or_else(|| DbError::Backend("stats flag query returned no rows".into()))?;
    let alpha = flags.get_i64_at(0)?;
    let lucky = flags.get_i64_at(1)?;
    let awakened = flags.get_i64_at(2)?;
    let imported = flags.get_i64_at(3)?;

    // The element histogram / human / predator / oilrig / summon counts run per
    // DISTINCT character_id (weighted by its row count), not per pal_data row.
    // ORDER BY MIN(id) walks groups in first-appearance order, so the
    // element_distribution key order matches a row-by-row scan's.
    let character_groups = db
        .query(
            "SELECT character_id, COUNT(*) FROM ups_pals
             GROUP BY character_id ORDER BY MIN(id)",
            &[],
        )
        .await?;
    let mut element_counts: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let (mut human, mut predator, mut oilrig, mut summon) = (0i64, 0i64, 0i64, 0i64);
    for group in &character_groups {
        let character_id = group.get_str_at(0)?;
        let rows = group.get_i64_at(1)?;
        if let Some(character_info) = pals_game_data.get(character_id) {
            if let Some(elements) = character_info
                .get("element_types")
                .and_then(|v| v.as_array())
            {
                for element in elements.iter().filter_map(|e| e.as_str()) {
                    let current = element_counts
                        .get(element)
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    element_counts.insert(element.to_string(), serde_json::json!(current + rows));
                }
            }
            if !character_info
                .get("is_pal")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
            {
                human += rows;
            }
        }
        let lower = character_id.to_lowercase();
        if lower.contains("predator_") {
            predator += rows;
        } else if lower.contains("_oilrig") {
            oilrig += rows;
        } else if lower.contains("summon_") {
            summon += rows;
        }
    }

    // COALESCE keeps the last known most_* ids when the table is empty rather than
    // nulling them out.
    db.execute(
        "UPDATE ups_stats SET
           total_pals = ?, total_collections = ?, total_tags = ?, total_transfers = ?,
           total_clones = ?, storage_size_mb = ?,
           most_transferred_pal_id = COALESCE(?, most_transferred_pal_id),
           most_cloned_pal_id = COALESCE(?, most_cloned_pal_id),
           most_popular_character_id = COALESCE(?, most_popular_character_id),
           element_distribution = ?, alpha_count = ?, lucky_count = ?, awakened_count = ?,
           imported_count = ?, human_count = ?,
           predator_count = ?, oilrig_count = ?, summon_count = ?, last_updated = ?
         WHERE id = 1",
        &[
            total_pals.into(),
            total_collections.into(),
            total_tags.into(),
            total_transfers.into(),
            total_clones.into(),
            storage_size_mb.into(),
            most_transferred.into(),
            most_cloned.into(),
            most_popular.into(),
            serde_json::Value::Object(element_counts).to_string().into(),
            alpha.into(),
            lucky.into(),
            awakened.into(),
            imported.into(),
            human.into(),
            predator.into(),
            oilrig.into(),
            summon.into(),
            crate::time::now_iso_utc_offset().into(),
        ],
    )
    .await?;
    Ok(())
}

pub async fn get_stats(
    db: &dyn crate::DbDriver,
    pals_game_data: &serde_json::Value,
) -> Result<UpsStatsRecord, DbError> {
    ensure_stats_row(db).await?;
    recompute_stats(db, pals_game_data).await?;
    let rows = db
        .query(
            "SELECT total_pals, total_collections, total_tags, total_transfers, total_clones,
                storage_size_mb, most_transferred_pal_id, most_cloned_pal_id,
                most_popular_character_id, element_distribution, alpha_count, lucky_count,
                awakened_count, imported_count,
                human_count, predator_count, oilrig_count, summon_count, last_updated
         FROM ups_stats WHERE id = 1",
            &[],
        )
        .await?;
    rows.first()
        .map(map_stats)
        .transpose()?
        .ok_or_else(|| DbError::Other("ups_stats row 1 missing".to_string()))
}

pub async fn update_collection_counts(db: &dyn crate::DbDriver) -> Result<(), DbError> {
    db.execute(
        "UPDATE ups_collections SET
           pal_count = (SELECT COUNT(*) FROM ups_pals WHERE ups_pals.collection_id = ups_collections.id),
           updated_at = ?",
        &[crate::time::now_iso_naive_utc().into()],
    )
    .await?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
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

fn map_collection(r: &crate::DbRow) -> Result<UpsCollectionRecord, DbError> {
    Ok(UpsCollectionRecord {
        id: r.get_i64("id")?,
        name: r.get_string("name")?,
        description: r.get_opt_str("description")?,
        color: r.get_opt_str("color")?,
        icon: r.get_opt_str("icon")?,
        is_favorite: r.get_bool("is_favorite")?,
        is_archived: r.get_bool("is_archived")?,
        pal_count: r.get_i64("pal_count")?,
        created_at: r.get_string("created_at")?,
        updated_at: r.get_string("updated_at")?,
    })
}

pub async fn create_collection(
    db: &dyn crate::DbDriver,
    name: &str,
    description: Option<&str>,
    color: Option<&str>,
) -> Result<UpsCollectionRecord, DbError> {
    let now = crate::time::now_iso_naive_utc();
    let id: i64 = crate::scalar_i64(
        &db.query(
            "INSERT INTO ups_collections (name, description, color, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?) RETURNING id",
            &[
                name.into(),
                description.into(),
                color.into(),
                now.clone().into(),
                now.clone().into(),
            ],
        )
        .await?,
    )?;
    Ok(get_collection_by_id(db, id)
        .await?
        .expect("row just inserted"))
}

pub async fn get_collection_by_id(
    db: &dyn crate::DbDriver,
    collection_id: i64,
) -> Result<Option<UpsCollectionRecord>, DbError> {
    let rows = db
        .query(
            "SELECT * FROM ups_collections WHERE id = ?",
            &[collection_id.into()],
        )
        .await?;
    rows.first().map(map_collection).transpose()
}

pub async fn get_collections(
    db: &dyn crate::DbDriver,
) -> Result<Vec<UpsCollectionRecord>, DbError> {
    let rows = db
        .query("SELECT * FROM ups_collections ORDER BY name", &[])
        .await?;
    rows.iter().map(map_collection).collect()
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
    db: &dyn crate::DbDriver,
    pal_id: i64,
    updates: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<UpsPalRecord>, DbError> {
    let Some(mut record) = get_pal_by_id(db, pal_id).await? else {
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

    db.execute(
        "UPDATE ups_pals SET instance_id = ?, character_id = ?, nickname = ?, level = ?,
           pal_data = ?, source_save_file = ?, source_player_uid = ?, source_player_name = ?,
           source_storage_type = ?, source_storage_slot = ?, collection_id = ?, tags = ?,
           notes = ?, updated_at = ?, last_accessed_at = ?, transfer_count = ?, clone_count = ?
         WHERE id = ?",
        &[
            record.instance_id.clone().into(),
            record.character_id.clone().into(),
            record.nickname.clone().into(),
            record.level.into(),
            record.pal_data.to_string().into(),
            record.source_save_file.clone().into(),
            record.source_player_uid.clone().into(),
            record.source_player_name.clone().into(),
            record.source_storage_type.clone().into(),
            record.source_storage_slot.into(),
            record.collection_id.into(),
            record.tags.to_string().into(),
            record.notes.clone().into(),
            record.updated_at.clone().into(),
            record.last_accessed_at.clone().into(),
            record.transfer_count.into(),
            record.clone_count.into(),
            pal_id.into(),
        ],
    )
    .await?;

    if updates.contains_key("collection_id") {
        update_collection_counts(db).await?;
    }
    Ok(Some(record))
}

pub async fn delete_pals(
    db: &dyn crate::DbDriver,
    pal_ids: &[i64],
    pals_game_data: &serde_json::Value,
) -> Result<i64, DbError> {
    // Which of the requested ids exist, fetched in chunked IN lists instead of
    // one SELECT per id.
    let mut existing: Vec<i64> = Vec::new();
    for chunk in pal_ids.chunks(SQL_CHUNK) {
        let mut builder = crate::SqlBuilder::new("SELECT id FROM ups_pals WHERE id IN (");
        let mut separated = builder.separated(", ");
        for pal_id in chunk {
            separated.push_bind(*pal_id);
        }
        builder.push(")");
        let (sql, params) = builder.into_parts();
        existing.extend(ids_from_rows(&db.query(&sql, &params).await?)?);
    }
    // Log (and count) each existing id once, in first-occurrence order — the
    // rows the per-id loop used to write.
    let existing_set: std::collections::HashSet<i64> = existing.iter().copied().collect();
    let mut seen = std::collections::HashSet::new();
    let to_delete: Vec<i64> = pal_ids
        .iter()
        .copied()
        .filter(|pal_id| existing_set.contains(pal_id) && seen.insert(*pal_id))
        .collect();

    if !to_delete.is_empty() {
        let mut statements = build_ups_log_rows(&to_delete, "delete");
        statements.extend(build_delete_statements(&to_delete));
        db.execute_batch(&as_batch(&statements)).await?;
    }
    recompute_stats(db, pals_game_data).await?;
    update_collection_counts(db).await?;
    Ok(to_delete.len() as i64)
}

pub async fn clone_pal(
    db: &dyn crate::DbDriver,
    pal_id: i64,
    pals_game_data: &serde_json::Value,
) -> Result<Option<UpsPalRecord>, DbError> {
    let Some(original) = get_pal_by_id(db, pal_id).await? else {
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
    let clone_id: i64 = crate::scalar_i64(
        &db.query(
            "INSERT INTO ups_pals
         (instance_id, character_id, nickname, level, pal_data, source_save_file,
          source_player_uid, source_player_name, source_storage_type, collection_id,
          tags, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'ups_clone', ?, ?, ?, ?, ?) RETURNING id",
            &[
                uuid::Uuid::new_v4().to_string().into(),
                original.character_id.clone().into(),
                clone_nickname.clone().into(),
                original.level.into(),
                original.pal_data.to_string().into(),
                original.source_save_file.clone().into(),
                original.source_player_uid.clone().into(),
                original.source_player_name.clone().into(),
                original.collection_id.into(),
                original.tags.to_string().into(),
                clone_notes.clone().into(),
                now.clone().into(),
                now.clone().into(),
            ],
        )
        .await?,
    )?;

    db.execute(
        "UPDATE ups_pals SET clone_count = clone_count + 1 WHERE id = ?",
        &[pal_id.into()],
    )
    .await?;
    recompute_stats(db, pals_game_data).await?;
    update_collection_counts(db).await?;
    log_transfer(
        db,
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
    get_pal_by_id(db, clone_id).await
}

pub async fn nuke_all_pals(
    db: &dyn crate::DbDriver,
    pals_game_data: &serde_json::Value,
) -> Result<i64, DbError> {
    let all_ids = ids_from_rows(&db.query("SELECT id FROM ups_pals", &[]).await?)?;
    if all_ids.is_empty() {
        return Ok(0);
    }
    // One multi-row log insert + the delete + the collection reset, committed
    // together — instead of one logged statement per pal.
    let mut statements = build_ups_log_rows(&all_ids, "nuke_delete");
    statements.push(("DELETE FROM ups_pals".to_string(), Vec::new()));
    statements.push((
        "UPDATE ups_collections SET pal_count = 0, updated_at = ?".to_string(),
        vec![crate::time::now_iso_naive_utc().into()],
    ));
    db.execute_batch(&as_batch(&statements)).await?;
    recompute_stats(db, pals_game_data).await?;
    Ok(all_ids.len() as i64)
}

#[derive(Debug, Clone, Default)]
pub struct ExportDestinationInfo {
    pub save_file_name: Option<String>,
    pub player_name: Option<String>,
    pub player_uid: Option<String>,
}

pub async fn export_pal_to_save(
    db: &dyn crate::DbDriver,
    pal_id: i64,
    destination_type: &str,
    destination: &ExportDestinationInfo,
) -> Result<bool, DbError> {
    if get_pal_by_id(db, pal_id).await?.is_none() {
        return Ok(false);
    }
    db.execute(
        "UPDATE ups_pals SET last_accessed_at = ?, transfer_count = transfer_count + 1 WHERE id = ?",
        &[crate::time::now_iso_utc_offset().into(), pal_id.into()],
    )
    .await?;
    log_transfer(
        db,
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
    db: &dyn crate::DbDriver,
    collection_id: i64,
    updates: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<UpsCollectionRecord>, DbError> {
    let Some(mut record) = get_collection_by_id(db, collection_id).await? else {
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
    db.execute(
        "UPDATE ups_collections SET name = ?, description = ?, color = ?, icon = ?,
           is_favorite = ?, is_archived = ?, pal_count = ?, updated_at = ? WHERE id = ?",
        &[
            record.name.clone().into(),
            record.description.clone().into(),
            record.color.clone().into(),
            record.icon.clone().into(),
            record.is_favorite.into(),
            record.is_archived.into(),
            record.pal_count.into(),
            record.updated_at.clone().into(),
            collection_id.into(),
        ],
    )
    .await?;
    Ok(Some(record))
}

pub async fn delete_collection(
    db: &dyn crate::DbDriver,
    collection_id: i64,
) -> Result<bool, DbError> {
    if get_collection_by_id(db, collection_id).await?.is_none() {
        return Ok(false);
    }
    db.execute(
        "UPDATE ups_pals SET collection_id = NULL WHERE collection_id = ?",
        &[collection_id.into()],
    )
    .await?;
    db.execute(
        "DELETE FROM ups_collections WHERE id = ?",
        &[collection_id.into()],
    )
    .await?;
    Ok(true)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UpsTagRecord {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub usage_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

fn map_tag(r: &crate::DbRow) -> Result<UpsTagRecord, DbError> {
    Ok(UpsTagRecord {
        id: r.get_i64("id")?,
        name: r.get_string("name")?,
        description: r.get_opt_str("description")?,
        color: r.get_opt_str("color")?,
        usage_count: r.get_i64("usage_count")?,
        created_at: r.get_string("created_at")?,
        updated_at: r.get_string("updated_at")?,
    })
}

pub async fn get_tag_by_id(
    db: &dyn crate::DbDriver,
    tag_id: i64,
) -> Result<Option<UpsTagRecord>, DbError> {
    let rows = db
        .query("SELECT * FROM ups_tags WHERE id = ?", &[tag_id.into()])
        .await?;
    rows.first().map(map_tag).transpose()
}

pub async fn get_available_tags(db: &dyn crate::DbDriver) -> Result<Vec<UpsTagRecord>, DbError> {
    let rows = db
        .query("SELECT * FROM ups_tags ORDER BY name", &[])
        .await?;
    rows.iter().map(map_tag).collect()
}

pub async fn create_or_update_tag(
    db: &dyn crate::DbDriver,
    name: &str,
    description: Option<&str>,
    color: Option<&str>,
) -> Result<UpsTagRecord, DbError> {
    let existing: Option<i64> = crate::opt_scalar_i64(
        &db.query("SELECT id FROM ups_tags WHERE name = ?", &[name.into()])
            .await?,
    )?;
    match existing {
        Some(tag_id) => {
            // COALESCE keeps the current value where None was passed, so one
            // statement replaces the former per-field updates.
            db.execute(
                "UPDATE ups_tags SET description = COALESCE(?, description),
                   color = COALESCE(?, color), updated_at = ? WHERE id = ?",
                &[
                    description.into(),
                    color.into(),
                    crate::time::now_iso_utc_offset().into(),
                    tag_id.into(),
                ],
            )
            .await?;
            Ok(get_tag_by_id(db, tag_id).await?.expect("existing tag"))
        }
        None => {
            let now = crate::time::now_iso_naive_utc();
            let tag_id: i64 = crate::scalar_i64(
                &db.query(
                    "INSERT INTO ups_tags (name, description, color, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?) RETURNING id",
                    &[
                        name.into(),
                        description.into(),
                        color.into(),
                        now.clone().into(),
                        now.clone().into(),
                    ],
                )
                .await?,
            )?;
            Ok(get_tag_by_id(db, tag_id).await?.expect("row just inserted"))
        }
    }
}

async fn rewrite_pal_tags(
    db: &dyn crate::DbDriver,
    tag_name: &str,
    replacement: Option<&str>,
) -> Result<(), DbError> {
    let encoded = serde_json::to_string(tag_name).expect("tag encodes");
    let rows: Vec<(i64, String)> = db
        .query(
            "SELECT id, tags FROM ups_pals WHERE tags LIKE ?",
            &[format!("%{encoded}%").into()],
        )
        .await?
        .iter()
        .map(|r| Ok((r.get_i64_at(0)?, r.get_str_at(1)?.to_string())))
        .collect::<Result<Vec<_>, DbError>>()?;
    // The rewritten tag arrays still have to be computed per row in Rust, but
    // all the UPDATEs commit as one batch.
    let mut updates: Vec<(String, Vec<crate::DbValue>)> = Vec::new();
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
        updates.push((
            "UPDATE ups_pals SET tags = ?, updated_at = ? WHERE id = ?".to_string(),
            vec![
                serde_json::Value::Array(rewritten).to_string().into(),
                crate::time::now_iso_utc_offset().into(),
                pal_id.into(),
            ],
        ));
    }
    if !updates.is_empty() {
        db.execute_batch(&as_batch(&updates)).await?;
    }
    Ok(())
}

pub async fn update_tag(
    db: &dyn crate::DbDriver,
    tag_id: i64,
    updates: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<UpsTagRecord>, DbError> {
    let Some(mut record) = get_tag_by_id(db, tag_id).await? else {
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
    db.execute(
        "UPDATE ups_tags SET name = ?, description = ?, color = ?, usage_count = ?, updated_at = ?
         WHERE id = ?",
        &[
            record.name.clone().into(),
            record.description.clone().into(),
            record.color.clone().into(),
            record.usage_count.into(),
            record.updated_at.clone().into(),
            tag_id.into(),
        ],
    )
    .await?;
    if updates.contains_key("name") && old_name != record.name {
        rewrite_pal_tags(db, &old_name, Some(&record.name)).await?;
    }
    Ok(Some(record))
}

pub async fn delete_tag(db: &dyn crate::DbDriver, tag_id: i64) -> Result<bool, DbError> {
    let Some(record) = get_tag_by_id(db, tag_id).await? else {
        return Ok(false);
    };
    rewrite_pal_tags(db, &record.name, None).await?;
    db.execute("DELETE FROM ups_tags WHERE id = ?", &[tag_id.into()])
        .await?;
    Ok(true)
}
