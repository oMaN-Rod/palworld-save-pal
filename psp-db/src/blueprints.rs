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

fn row_from(row: &crate::DbRow) -> Result<BlueprintRow, DbError> {
    Ok(BlueprintRow {
        id: row.get_string("id")?,
        name: row.get_string("name")?,
        source_world: row.get_string("source_world")?,
        source_base: row.get_string("source_base")?,
        created_at: row.get_i64("created_at")?,
        schema_version: row.get_i64("schema_version")?,
        structure_count: row.get_i64("structure_count")?,
        manifest: row.get_string("manifest")?,
        footprint_radius: row.get_f64("footprint_radius")?,
    })
}

pub async fn insert(db: &dyn crate::DbDriver, blueprint: NewBlueprint) -> Result<String, DbError> {
    let id = blueprint.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    db.execute(
        "INSERT INTO blueprints
         (id, name, source_world, source_base, created_at, schema_version,
          structure_count, manifest, footprint_radius, payload, preview)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            id.clone().into(), blueprint.name.into(), blueprint.source_world.into(),
            blueprint.source_base.into(), blueprint.created_at.into(),
            blueprint.schema_version.into(), blueprint.structure_count.into(),
            blueprint.manifest.into(), blueprint.footprint_radius.into(),
            blueprint.payload.into(), blueprint.preview.into(),
        ],
    ).await?;
    Ok(id)
}

pub async fn list(db: &dyn crate::DbDriver) -> Result<Vec<BlueprintRow>, DbError> {
    let rows = db.query(
        "SELECT id, name, source_world, source_base, created_at, schema_version,
                structure_count, manifest, footprint_radius
         FROM blueprints
         ORDER BY created_at DESC, rowid DESC",
        &[],
    ).await?;
    rows.iter().map(row_from).collect()
}

pub async fn get(db: &dyn crate::DbDriver, id: &str) -> Result<Option<StoredBlueprint>, DbError> {
    let rows = db.query("SELECT * FROM blueprints WHERE id = ?", &[id.into()]).await?;
    match rows.first() {
        Some(row) => Ok(Some(StoredBlueprint { row: row_from(row)?, payload: row.get_blob("payload")? })),
        None => Ok(None),
    }
}

pub async fn delete(db: &dyn crate::DbDriver, id: &str) -> Result<bool, DbError> {
    let n = db.execute("DELETE FROM blueprints WHERE id = ?", &[id.into()]).await?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_driver() -> crate::SqlxSqliteDriver {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::open(&dir.path().join("psp-rs.db")).await.unwrap();
        std::mem::forget(dir);
        crate::SqlxSqliteDriver::new(pool)
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
        let db = test_driver().await;
        let id = insert(&db, sample("Farm", &[1, 2, 3, 4])).await.unwrap();
        assert!(!id.is_empty());

        let rows = list(&db).await.unwrap();
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
        let db = test_driver().await;
        let id = insert(&db, sample("Farm", &[9, 8, 7, 6, 5]))
            .await
            .unwrap();

        let stored = get(&db, &id).await.unwrap().expect("row present");
        assert_eq!(stored.row.id, id);
        assert_eq!(
            stored.payload,
            vec![9, 8, 7, 6, 5],
            "payload round-trips byte-for-byte"
        );

        assert!(get(&db, "no-such-id").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_removes_the_row() {
        let db = test_driver().await;
        let id = insert(&db, sample("Farm", &[1, 2, 3])).await.unwrap();

        assert!(delete(&db, &id).await.unwrap(), "deleting an existing row reports success");
        assert!(get(&db, &id).await.unwrap().is_none(), "the row is gone");
        assert!(list(&db).await.unwrap().is_empty(), "list no longer shows it");
        assert!(!delete(&db, "no-such-id").await.unwrap(), "deleting a missing row reports false");
    }
}
