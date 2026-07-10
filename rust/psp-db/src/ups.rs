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
        // Build the OR group; "human" with an empty id list contributes nothing
        // (ups.py:185-191); a fully empty group adds no condition (ups.py:207-208).
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
