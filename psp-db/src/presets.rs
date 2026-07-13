use sqlx::{Row, SqlitePool};

use crate::error::DbError;

const CONTAINER_COLUMNS: [&str; 7] = [
    "skills",
    "common_container",
    "essential_container",
    "weapon_load_out_container",
    "player_equipment_armor_container",
    "food_equip_container",
    "storage_container",
];

fn json_or_null(text: Option<String>) -> serde_json::Value {
    text.and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// id -> wire preset object, in table insertion order (which survives only because
/// serde_json is built with the `preserve_order` feature).
pub async fn get_all(
    pool: &SqlitePool,
) -> Result<serde_json::Map<String, serde_json::Value>, DbError> {
    let rows = sqlx::query("SELECT * FROM presets ORDER BY rowid")
        .fetch_all(pool)
        .await?;
    let mut result = serde_json::Map::new();
    for row in rows {
        let id: String = row.try_get("id")?;
        let mut preset = serde_json::Map::new();
        preset.insert("id".into(), serde_json::json!(id));
        preset.insert(
            "name".into(),
            serde_json::json!(row.try_get::<String, _>("name")?),
        );
        preset.insert(
            "type".into(),
            serde_json::json!(row.try_get::<String, _>("preset_type")?),
        );
        for column in CONTAINER_COLUMNS {
            preset.insert(
                column.into(),
                json_or_null(row.try_get::<Option<String>, _>(column)?),
            );
        }
        let pal_preset = json_or_null(row.try_get::<Option<String>, _>("pal_preset")?);
        let pal_preset_id = pal_preset
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| serde_json::json!(s))
            .unwrap_or(serde_json::Value::Null);
        preset.insert("pal_preset_id".into(), pal_preset_id);
        // The wire contract omits `pal_preset` entirely when unset — it is never sent as null.
        if pal_preset.is_object() {
            preset.insert("pal_preset".into(), pal_preset);
        }
        result.insert(id, serde_json::Value::Object(preset));
    }
    Ok(result)
}

/// Inserts a preset from a wire object, honouring an `id` the payload already carries
/// (seed rows do) and generating one otherwise. Returns the preset id.
pub async fn add(pool: &SqlitePool, preset_data: serde_json::Value) -> Result<String, DbError> {
    let object = preset_data
        .as_object()
        .ok_or_else(|| DbError::Other("preset payload is not an object".into()))?;
    let preset_id = object
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let name = object
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| DbError::Other("preset missing name".into()))?
        .to_string();
    let preset_type = object
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| DbError::Other("preset missing type".into()))?
        .to_string();

    let column_json = |key: &str| -> Option<String> {
        object
            .get(key)
            .filter(|v| !v.is_null())
            .map(|v| v.to_string())
    };
    let pal_preset_json = match object.get("pal_preset").filter(|v| !v.is_null()) {
        Some(pal_preset) => {
            let mut pal_object = pal_preset
                .as_object()
                .cloned()
                .ok_or_else(|| DbError::Other("pal_preset is not an object".into()))?;
            if !pal_object.get("id").map(|v| v.is_string()).unwrap_or(false) {
                pal_object.insert(
                    "id".into(),
                    serde_json::json!(uuid::Uuid::new_v4().to_string()),
                );
            }
            Some(serde_json::Value::Object(pal_object).to_string())
        }
        None => None,
    };

    sqlx::query(
        "INSERT INTO presets
         (id, name, preset_type, skills, common_container, essential_container,
          weapon_load_out_container, player_equipment_armor_container,
          food_equip_container, storage_container, pal_preset)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&preset_id)
    .bind(name)
    .bind(preset_type)
    .bind(column_json("skills"))
    .bind(column_json("common_container"))
    .bind(column_json("essential_container"))
    .bind(column_json("weapon_load_out_container"))
    .bind(column_json("player_equipment_armor_container"))
    .bind(column_json("food_equip_container"))
    .bind(column_json("storage_container"))
    .bind(pal_preset_json)
    .execute(pool)
    .await?;
    Ok(preset_id)
}

pub async fn update_name(
    pool: &SqlitePool,
    preset_id: &str,
    new_name: &str,
) -> Result<bool, DbError> {
    let result = sqlx::query("UPDATE presets SET name = ? WHERE id = ?")
        .bind(new_name)
        .bind(preset_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete(pool: &SqlitePool, preset_id: &str) -> Result<bool, DbError> {
    let result = sqlx::query("DELETE FROM presets WHERE id = ?")
        .bind(preset_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn nuke(pool: &SqlitePool) -> Result<(), DbError> {
    sqlx::query("DELETE FROM presets").execute(pool).await?;
    Ok(())
}

/// Seeds the presets table from the bundled JSON, but only when it is empty — a
/// user who deleted every seeded preset does not get them back.
pub async fn populate_from_json(
    pool: &SqlitePool,
    presets_seed: &serde_json::Value,
) -> Result<(), DbError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM presets")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }
    if let Some(entries) = presets_seed.as_array() {
        for entry in entries {
            add(pool, entry.clone()).await?;
        }
    }
    Ok(())
}
