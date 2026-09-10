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
    db: &dyn crate::DbDriver,
) -> Result<serde_json::Map<String, serde_json::Value>, DbError> {
    let rows = db.query("SELECT * FROM presets ORDER BY rowid", &[]).await?;
    let mut result = serde_json::Map::new();
    for row in &rows {
        let id = row.get_string("id")?;
        let mut preset = serde_json::Map::new();
        preset.insert("id".into(), serde_json::json!(id));
        preset.insert("name".into(), serde_json::json!(row.get_string("name")?));
        preset.insert(
            "type".into(),
            serde_json::json!(row.get_string("preset_type")?),
        );
        for column in CONTAINER_COLUMNS {
            preset.insert(column.into(), json_or_null(row.get_opt_str(column)?));
        }
        let pal_preset = json_or_null(row.get_opt_str("pal_preset")?);
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

/// Honours an `id` the payload already carries (seed rows do); generates one otherwise.
pub async fn add(
    db: &dyn crate::DbDriver,
    preset_data: serde_json::Value,
) -> Result<String, DbError> {
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

    db.execute(
        "INSERT INTO presets
         (id, name, preset_type, skills, common_container, essential_container,
          weapon_load_out_container, player_equipment_armor_container,
          food_equip_container, storage_container, pal_preset)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            preset_id.clone().into(),
            name.into(),
            preset_type.into(),
            column_json("skills").into(),
            column_json("common_container").into(),
            column_json("essential_container").into(),
            column_json("weapon_load_out_container").into(),
            column_json("player_equipment_armor_container").into(),
            column_json("food_equip_container").into(),
            column_json("storage_container").into(),
            pal_preset_json.into(),
        ],
    )
    .await?;
    Ok(preset_id)
}

pub async fn update_name(
    db: &dyn crate::DbDriver,
    preset_id: &str,
    new_name: &str,
) -> Result<bool, DbError> {
    let n = db
        .execute(
            "UPDATE presets SET name = ? WHERE id = ?",
            &[new_name.into(), preset_id.into()],
        )
        .await?;
    Ok(n > 0)
}

pub async fn delete(db: &dyn crate::DbDriver, preset_id: &str) -> Result<bool, DbError> {
    let n = db
        .execute("DELETE FROM presets WHERE id = ?", &[preset_id.into()])
        .await?;
    Ok(n > 0)
}

pub async fn nuke(db: &dyn crate::DbDriver) -> Result<(), DbError> {
    db.execute("DELETE FROM presets", &[]).await?;
    Ok(())
}

/// Seeds the presets table from the bundled JSON, but only when it is empty — a
/// user who deleted every seeded preset does not get them back.
pub async fn populate_from_json(
    db: &dyn crate::DbDriver,
    presets_seed: &serde_json::Value,
) -> Result<(), DbError> {
    let count = crate::scalar_i64(&db.query("SELECT COUNT(*) FROM presets", &[]).await?)?;
    if count > 0 {
        return Ok(());
    }
    if let Some(entries) = presets_seed.as_array() {
        for entry in entries {
            add(db, entry.clone()).await?;
        }
    }
    Ok(())
}
