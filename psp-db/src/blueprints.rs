use sqlx::{Row, SqlitePool};

use crate::error::DbError;

pub struct NewBlueprint {
    pub id: Option<String>,
    pub name: String,
    pub source_world: String,
    pub source_base: String,
    pub created_at: i64,
    pub schema_version: i64,
    pub structure_count: i64,
    pub manifest: String,
    pub footprint_radius: f64,
    pub payload: Vec<u8>,
    pub preview: Option<Vec<u8>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BlueprintRow {
    pub id: String,
    pub name: String,
    pub source_world: String,
    pub source_base: String,
    pub created_at: i64,
    pub schema_version: i64,
    pub structure_count: i64,
    pub manifest: String,
    pub footprint_radius: f64,
}

pub struct StoredBlueprint {
    pub row: BlueprintRow,
    pub payload: Vec<u8>,
}

fn row_from(row: &sqlx::sqlite::SqliteRow) -> Result<BlueprintRow, DbError> {
    Ok(BlueprintRow {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        source_world: row.try_get("source_world")?,
        source_base: row.try_get("source_base")?,
        created_at: row.try_get("created_at")?,
        schema_version: row.try_get("schema_version")?,
        structure_count: row.try_get("structure_count")?,
        manifest: row.try_get("manifest")?,
        footprint_radius: row.try_get("footprint_radius")?,
    })
}

pub async fn insert(pool: &SqlitePool, blueprint: NewBlueprint) -> Result<String, DbError> {
    let id = blueprint
        .id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    sqlx::query(
        "INSERT INTO blueprints
         (id, name, source_world, source_base, created_at, schema_version,
          structure_count, manifest, footprint_radius, payload, preview)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(blueprint.name)
    .bind(blueprint.source_world)
    .bind(blueprint.source_base)
    .bind(blueprint.created_at)
    .bind(blueprint.schema_version)
    .bind(blueprint.structure_count)
    .bind(blueprint.manifest)
    .bind(blueprint.footprint_radius)
    .bind(blueprint.payload)
    .bind(blueprint.preview)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<BlueprintRow>, DbError> {
    let rows = sqlx::query(
        "SELECT id, name, source_world, source_base, created_at, schema_version,
                structure_count, manifest, footprint_radius
         FROM blueprints
         ORDER BY created_at DESC, rowid DESC",
    )
    .fetch_all(pool)
    .await?;
    rows.iter().map(row_from).collect()
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<StoredBlueprint>, DbError> {
    let row = sqlx::query("SELECT * FROM blueprints WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    match row {
        Some(row) => Ok(Some(StoredBlueprint {
            row: row_from(&row)?,
            payload: row.try_get("payload")?,
        })),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> sqlx::SqlitePool {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::open(&dir.path().join("psp-rs.db")).await.unwrap();
        std::mem::forget(dir);
        pool
    }

    fn sample(name: &str, payload: &[u8]) -> NewBlueprint {
        NewBlueprint {
            id: None,
            name: name.to_string(),
            source_world: "Autosave_W".to_string(),
            source_base: "Home".to_string(),
            created_at: 1_700_000_000,
            schema_version: 1,
            structure_count: 42,
            manifest: r#"{"production_config":true}"#.to_string(),
            footprint_radius: 3500.0,
            payload: payload.to_vec(),
            preview: None,
        }
    }

    #[tokio::test]
    async fn insert_then_list_returns_header_columns_without_the_payload() {
        let pool = test_pool().await;
        let id = insert(&pool, sample("Farm", &[1, 2, 3, 4])).await.unwrap();
        assert!(!id.is_empty());

        let rows = list(&pool).await.unwrap();
        assert_eq!(rows.len(), 1, "exactly one blueprint was inserted");
        let row = &rows[0];
        assert_eq!(row.id, id);
        assert_eq!(row.name, "Farm");
        assert_eq!(row.source_world, "Autosave_W");
        assert_eq!(row.structure_count, 42);
        assert_eq!(row.schema_version, 1);
        assert_eq!(row.footprint_radius, 3500.0);
        // BlueprintRow has no payload field at all — the list path never reads the blob.
    }

    #[tokio::test]
    async fn get_returns_the_payload_bytes_verbatim() {
        let pool = test_pool().await;
        let id = insert(&pool, sample("Farm", &[9, 8, 7, 6, 5]))
            .await
            .unwrap();

        let stored = get(&pool, &id).await.unwrap().expect("row present");
        assert_eq!(stored.row.id, id);
        assert_eq!(
            stored.payload,
            vec![9, 8, 7, 6, 5],
            "payload round-trips byte-for-byte"
        );

        assert!(get(&pool, "no-such-id").await.unwrap().is_none());
    }
}
