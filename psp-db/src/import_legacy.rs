use std::path::{Path, PathBuf};

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, Row, SqliteConnection, SqlitePool};

use crate::error::DbError;

pub const LEGACY_IMPORT_META_KEY: &str = "legacy_import";

#[derive(Debug)]
pub struct LegacyImportReport {
    pub backup_path: PathBuf,
    pub settings_imported: bool,
    pub presets_imported: u32,
    pub ups_collections_imported: u32,
    pub ups_tags_imported: u32,
    pub ups_pals_imported: u32,
    pub ups_pals_skipped: u32,
    pub ups_transfer_log_imported: u32,
    pub ups_stats_imported: bool,
    pub servers_imported: u32,
}

pub type PalDataValidator<'a> =
    &'a (dyn Fn(&serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync);

/// The legacy DB stores DATETIME as space-separated text ("2026-01-02 03:04:05.123456");
/// this DB stores ISO ("2026-01-02T03:04:05.123456").
fn legacy_datetime_to_iso(value: Option<String>) -> Option<String> {
    value.map(|v| v.replacen(' ', "T", 1))
}

/// The legacy DB stores gender as the enum member NAME ("MALE"); the wire uses the
/// value ("Male").
fn gender_name_to_value(name: Option<String>) -> serde_json::Value {
    match name.as_deref() {
        Some("NONE") => serde_json::Value::String("None".to_string()),
        Some("MALE") => serde_json::Value::String("Male".to_string()),
        Some("FEMALE") => serde_json::Value::String("Female".to_string()),
        Some(other) => serde_json::Value::String(other.to_string()),
        None => serde_json::Value::Null,
    }
}

fn parse_json_column(text: Option<String>) -> serde_json::Value {
    match text {
        Some(t) => serde_json::from_str(&t).unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::Null,
    }
}

/// Legacy UUID columns are dashless TEXT; this DB and the wire use canonical
/// dashed-lowercase. Non-UUID strings fall back to a lowercased copy rather than
/// being dropped.
fn normalize_uuid(raw: String) -> String {
    uuid::Uuid::parse_str(&raw)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| raw.to_lowercase())
}

async fn legacy_table_exists(conn: &mut SqliteConnection, table: &str) -> Result<bool, DbError> {
    let found: Option<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(table)
            .fetch_optional(&mut *conn)
            .await?;
    Ok(found.is_some())
}

pub async fn import_legacy_if_needed(
    pool: &SqlitePool,
    legacy_db_path: &Path,
    validate_pal_data: PalDataValidator<'_>,
) -> Result<Option<LegacyImportReport>, DbError> {
    if crate::meta::get(pool, LEGACY_IMPORT_META_KEY)
        .await?
        .is_some()
    {
        return Ok(None);
    }
    if !legacy_db_path.exists() {
        return Ok(None);
    }

    // The legacy file is only ever copied and read, never written to.
    let backup_path = legacy_db_path.with_extension("db.pre-rust-import.bak");
    if !backup_path.exists() {
        std::fs::copy(legacy_db_path, &backup_path)?;
    }

    let mut legacy = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(legacy_db_path)
            .read_only(true),
    )
    .await?;

    let mut report = LegacyImportReport {
        backup_path,
        settings_imported: false,
        presets_imported: 0,
        ups_collections_imported: 0,
        ups_tags_imported: 0,
        ups_pals_imported: 0,
        ups_pals_skipped: 0,
        ups_transfer_log_imported: 0,
        ups_stats_imported: false,
        servers_imported: 0,
    };

    if legacy_table_exists(&mut legacy, "settingsmodel").await? {
        if let Some(row) = sqlx::query("SELECT * FROM settingsmodel WHERE id = 1")
            .fetch_optional(&mut legacy)
            .await?
        {
            sqlx::query(
                "INSERT INTO settings (id, language, save_dir, clone_prefix, new_pal_prefix, debug_mode, cheat_mode)
                 VALUES (1, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                   language = excluded.language, save_dir = excluded.save_dir,
                   clone_prefix = excluded.clone_prefix, new_pal_prefix = excluded.new_pal_prefix,
                   debug_mode = excluded.debug_mode, cheat_mode = excluded.cheat_mode",
            )
            .bind(row.try_get::<String, _>("language")?)
            .bind(row.try_get::<String, _>("save_dir")?)
            .bind(row.try_get::<String, _>("clone_prefix")?)
            .bind(row.try_get::<String, _>("new_pal_prefix")?)
            .bind(row.try_get::<bool, _>("debug_mode")?)
            .bind(row.try_get::<bool, _>("cheat_mode").unwrap_or(false))
            .execute(pool)
            .await?;
            report.settings_imported = true;
        }
    }

    // The legacy DB splits a preset across `presetprofile` and a linked `palpreset` row;
    // here the pal half is folded into the `presets.pal_preset` JSON column.
    if legacy_table_exists(&mut legacy, "presetprofile").await? {
        let preset_rows = sqlx::query("SELECT * FROM presetprofile ORDER BY rowid")
            .fetch_all(&mut legacy)
            .await?;
        for preset_row in preset_rows {
            let pal_preset_id: Option<String> = preset_row.try_get("pal_preset_id").ok().flatten();
            let mut pal_preset_json: Option<String> = None;
            if let Some(ref linked_id) = pal_preset_id {
                if legacy_table_exists(&mut legacy, "palpreset").await? {
                    if let Some(pp) = sqlx::query("SELECT * FROM palpreset WHERE id = ?")
                        .bind(linked_id)
                        .fetch_optional(&mut legacy)
                        .await?
                    {
                        let mut obj = serde_json::Map::new();
                        obj.insert(
                            "id".into(),
                            serde_json::json!(pp.try_get::<String, _>("id")?),
                        );
                        obj.insert(
                            "lock".into(),
                            serde_json::json!(pp.try_get::<bool, _>("lock")?),
                        );
                        obj.insert(
                            "lock_element".into(),
                            serde_json::json!(pp
                                .try_get::<bool, _>("lock_element")
                                .unwrap_or(false)),
                        );
                        for text_col in ["element", "character_id", "nickname", "filtered_nickname"]
                        {
                            obj.insert(
                                text_col.into(),
                                serde_json::json!(pp
                                    .try_get::<Option<String>, _>(text_col)
                                    .ok()
                                    .flatten()),
                            );
                        }
                        for bool_col in ["is_lucky", "is_boss"] {
                            obj.insert(
                                bool_col.into(),
                                serde_json::json!(pp
                                    .try_get::<Option<bool>, _>(bool_col)
                                    .ok()
                                    .flatten()),
                            );
                        }
                        obj.insert(
                            "gender".into(),
                            gender_name_to_value(
                                pp.try_get::<Option<String>, _>("gender").ok().flatten(),
                            ),
                        );
                        for int_col in [
                            "rank_hp",
                            "rank_attack",
                            "rank_defense",
                            "rank_craftspeed",
                            "talent_hp",
                            "talent_shot",
                            "talent_defense",
                            "rank",
                            "level",
                            "exp",
                            "hp",
                            "friendship_point",
                        ] {
                            obj.insert(
                                int_col.into(),
                                serde_json::json!(pp
                                    .try_get::<Option<i64>, _>(int_col)
                                    .ok()
                                    .flatten()),
                            );
                        }
                        for json_col in [
                            "learned_skills",
                            "active_skills",
                            "passive_skills",
                            "work_suitability",
                        ] {
                            obj.insert(
                                json_col.into(),
                                parse_json_column(
                                    pp.try_get::<Option<String>, _>(json_col).ok().flatten(),
                                ),
                            );
                        }
                        for float_col in ["sanity", "stomach"] {
                            obj.insert(
                                float_col.into(),
                                serde_json::json!(pp
                                    .try_get::<Option<f64>, _>(float_col)
                                    .ok()
                                    .flatten()),
                            );
                        }
                        pal_preset_json = Some(serde_json::Value::Object(obj).to_string());
                    }
                }
            }

            let json_text = |col: &str| -> Option<String> {
                preset_row
                    .try_get::<Option<String>, _>(col)
                    .ok()
                    .flatten()
                    .and_then(|t| {
                        serde_json::from_str::<serde_json::Value>(&t)
                            .ok()
                            .map(|v| v.to_string())
                    })
            };

            sqlx::query(
                "INSERT OR REPLACE INTO presets
                 (id, name, preset_type, skills, common_container, essential_container,
                  weapon_load_out_container, player_equipment_armor_container,
                  food_equip_container, storage_container, pal_preset)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(preset_row.try_get::<String, _>("id")?)
            .bind(preset_row.try_get::<String, _>("name")?)
            .bind(preset_row.try_get::<String, _>("type")?)
            .bind(json_text("skills"))
            .bind(json_text("common_container"))
            .bind(json_text("essential_container"))
            .bind(json_text("weapon_load_out_container"))
            .bind(json_text("player_equipment_armor_container"))
            .bind(json_text("food_equip_container"))
            .bind(json_text("storage_container"))
            .bind(pal_preset_json)
            .execute(pool)
            .await?;
            report.presets_imported += 1;
        }
    }

    // Collections must land before pals: ups_pals.collection_id is a foreign key into them.
    if legacy_table_exists(&mut legacy, "ups_collections").await? {
        for row in sqlx::query("SELECT * FROM ups_collections ORDER BY id")
            .fetch_all(&mut legacy)
            .await?
        {
            sqlx::query(
                "INSERT OR REPLACE INTO ups_collections
                 (id, name, description, color, icon, is_favorite, is_archived, pal_count, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(row.try_get::<i64, _>("id")?)
            .bind(row.try_get::<String, _>("name")?)
            .bind(row.try_get::<Option<String>, _>("description")?)
            .bind(row.try_get::<Option<String>, _>("color")?)
            .bind(row.try_get::<Option<String>, _>("icon")?)
            .bind(row.try_get::<bool, _>("is_favorite")?)
            .bind(row.try_get::<bool, _>("is_archived")?)
            .bind(row.try_get::<i64, _>("pal_count")?)
            .bind(legacy_datetime_to_iso(row.try_get::<Option<String>, _>("created_at")?))
            .bind(legacy_datetime_to_iso(row.try_get::<Option<String>, _>("updated_at")?))
            .execute(pool)
            .await?;
            report.ups_collections_imported += 1;
        }
    }

    if legacy_table_exists(&mut legacy, "ups_tags").await? {
        for row in sqlx::query("SELECT * FROM ups_tags ORDER BY id")
            .fetch_all(&mut legacy)
            .await?
        {
            sqlx::query(
                "INSERT OR REPLACE INTO ups_tags
                 (id, name, description, color, usage_count, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(row.try_get::<i64, _>("id")?)
            .bind(row.try_get::<String, _>("name")?)
            .bind(row.try_get::<Option<String>, _>("description")?)
            .bind(row.try_get::<Option<String>, _>("color")?)
            .bind(row.try_get::<i64, _>("usage_count")?)
            .bind(legacy_datetime_to_iso(
                row.try_get::<Option<String>, _>("created_at")?,
            ))
            .bind(legacy_datetime_to_iso(
                row.try_get::<Option<String>, _>("updated_at")?,
            ))
            .execute(pool)
            .await?;
            report.ups_tags_imported += 1;
        }
    }

    if legacy_table_exists(&mut legacy, "ups_pals").await? {
        for row in sqlx::query("SELECT * FROM ups_pals ORDER BY id")
            .fetch_all(&mut legacy)
            .await?
        {
            let raw_pal_data = parse_json_column(row.try_get::<Option<String>, _>("pal_data")?);
            let normalized = match validate_pal_data(&raw_pal_data) {
                Ok(value) => value,
                Err(reason) => {
                    tracing::warn!(
                        pal_id = row.try_get::<i64, _>("id").unwrap_or(-1),
                        %reason,
                        "skipping legacy UPS pal with invalid pal_data"
                    );
                    report.ups_pals_skipped += 1;
                    continue;
                }
            };
            let tags = parse_json_column(row.try_get::<Option<String>, _>("tags")?);
            let tags = if tags.is_array() {
                tags
            } else {
                serde_json::json!([])
            };
            sqlx::query(
                "INSERT OR REPLACE INTO ups_pals
                 (id, instance_id, character_id, nickname, level, pal_data,
                  source_save_file, source_player_uid, source_player_name,
                  source_storage_type, source_storage_slot, collection_id, tags, notes,
                  created_at, updated_at, last_accessed_at, transfer_count, clone_count)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(row.try_get::<i64, _>("id")?)
            .bind(normalize_uuid(row.try_get::<String, _>("instance_id")?))
            .bind(row.try_get::<String, _>("character_id")?)
            .bind(row.try_get::<Option<String>, _>("nickname")?)
            .bind(row.try_get::<i64, _>("level")?)
            .bind(normalized.to_string())
            .bind(row.try_get::<Option<String>, _>("source_save_file")?)
            .bind(
                row.try_get::<Option<String>, _>("source_player_uid")?
                    .map(normalize_uuid),
            )
            .bind(row.try_get::<Option<String>, _>("source_player_name")?)
            .bind(row.try_get::<Option<String>, _>("source_storage_type")?)
            .bind(row.try_get::<Option<i64>, _>("source_storage_slot")?)
            .bind(row.try_get::<Option<i64>, _>("collection_id")?)
            .bind(tags.to_string())
            .bind(row.try_get::<Option<String>, _>("notes")?)
            .bind(legacy_datetime_to_iso(
                row.try_get::<Option<String>, _>("created_at")?,
            ))
            .bind(legacy_datetime_to_iso(
                row.try_get::<Option<String>, _>("updated_at")?,
            ))
            .bind(legacy_datetime_to_iso(
                row.try_get::<Option<String>, _>("last_accessed_at")?,
            ))
            .bind(row.try_get::<i64, _>("transfer_count")?)
            .bind(row.try_get::<i64, _>("clone_count")?)
            .execute(pool)
            .await?;
            report.ups_pals_imported += 1;
        }
    }

    if legacy_table_exists(&mut legacy, "ups_transfer_log").await? {
        for row in sqlx::query("SELECT * FROM ups_transfer_log ORDER BY id")
            .fetch_all(&mut legacy)
            .await?
        {
            sqlx::query(
                "INSERT OR REPLACE INTO ups_transfer_log
                 (id, pal_id, operation_type, source_type, source_location, destination_type,
                  destination_location, save_file_name, player_name, player_uid, success,
                  error_message, timestamp)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(row.try_get::<i64, _>("id")?)
            .bind(row.try_get::<i64, _>("pal_id")?)
            .bind(row.try_get::<String, _>("operation_type")?)
            .bind(row.try_get::<Option<String>, _>("source_type")?)
            .bind(row.try_get::<Option<String>, _>("source_location")?)
            .bind(row.try_get::<Option<String>, _>("destination_type")?)
            .bind(row.try_get::<Option<String>, _>("destination_location")?)
            .bind(row.try_get::<Option<String>, _>("save_file_name")?)
            .bind(row.try_get::<Option<String>, _>("player_name")?)
            .bind(
                row.try_get::<Option<String>, _>("player_uid")?
                    .map(normalize_uuid),
            )
            .bind(row.try_get::<bool, _>("success")?)
            .bind(row.try_get::<Option<String>, _>("error_message")?)
            .bind(legacy_datetime_to_iso(
                row.try_get::<Option<String>, _>("timestamp")?,
            ))
            .execute(pool)
            .await?;
            report.ups_transfer_log_imported += 1;
        }
    }

    // Copied verbatim; the first get_stats call recomputes every column anyway.
    if legacy_table_exists(&mut legacy, "ups_stats").await? {
        if let Some(row) = sqlx::query("SELECT * FROM ups_stats WHERE id = 1")
            .fetch_optional(&mut legacy)
            .await?
        {
            sqlx::query(
                "INSERT OR REPLACE INTO ups_stats
                 (id, total_pals, total_collections, total_tags, total_transfers, total_clones,
                  storage_size_mb, most_transferred_pal_id, most_cloned_pal_id,
                  most_popular_character_id, element_distribution, alpha_count, lucky_count,
                  human_count, predator_count, oilrig_count, summon_count, last_updated)
                 VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(row.try_get::<i64, _>("total_pals")?)
            .bind(row.try_get::<i64, _>("total_collections")?)
            .bind(row.try_get::<i64, _>("total_tags")?)
            .bind(row.try_get::<i64, _>("total_transfers")?)
            .bind(row.try_get::<i64, _>("total_clones")?)
            .bind(row.try_get::<f64, _>("storage_size_mb")?)
            .bind(row.try_get::<Option<i64>, _>("most_transferred_pal_id")?)
            .bind(row.try_get::<Option<i64>, _>("most_cloned_pal_id")?)
            .bind(row.try_get::<Option<String>, _>("most_popular_character_id")?)
            .bind(
                row.try_get::<String, _>("element_distribution")
                    .unwrap_or_else(|_| "{}".into()),
            )
            .bind(row.try_get::<i64, _>("alpha_count").unwrap_or(0))
            .bind(row.try_get::<i64, _>("lucky_count").unwrap_or(0))
            .bind(row.try_get::<i64, _>("human_count").unwrap_or(0))
            .bind(row.try_get::<i64, _>("predator_count").unwrap_or(0))
            .bind(row.try_get::<i64, _>("oilrig_count").unwrap_or(0))
            .bind(row.try_get::<i64, _>("summon_count").unwrap_or(0))
            .bind(
                legacy_datetime_to_iso(row.try_get::<Option<String>, _>("last_updated")?)
                    .unwrap_or_else(crate::time::now_iso_naive_utc),
            )
            .execute(pool)
            .await?;
            report.ups_stats_imported = true;
        }
    }

    if legacy_table_exists(&mut legacy, "servers").await? {
        for row in sqlx::query("SELECT * FROM servers ORDER BY id")
            .fetch_all(&mut legacy)
            .await?
        {
            let env_vars = parse_json_column(row.try_get::<Option<String>, _>("env_vars")?);
            let env_vars = if env_vars.is_object() {
                env_vars
            } else {
                serde_json::json!({})
            };
            sqlx::query(
                "INSERT OR REPLACE INTO servers
                 (id, name, container_name, image_name, server_type, game_port, query_port,
                  rest_api_port, data_volume_name, saves_path, mods_path, logicmods_path,
                  nativemods_path, install_path, steamcmd_path, pid, launch_args, workshop_dir,
                  server_name, server_description, server_password, admin_password, max_players,
                  env_vars, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(row.try_get::<i64, _>("id")?)
            .bind(row.try_get::<String, _>("name")?)
            .bind(row.try_get::<String, _>("container_name")?)
            .bind(row.try_get::<String, _>("image_name")?)
            .bind(row.try_get::<String, _>("server_type").unwrap_or_else(|_| "docker".into()))
            .bind(row.try_get::<i64, _>("game_port")?)
            .bind(row.try_get::<i64, _>("query_port")?)
            .bind(row.try_get::<i64, _>("rest_api_port")?)
            .bind(row.try_get::<String, _>("data_volume_name").unwrap_or_default())
            .bind(row.try_get::<String, _>("saves_path").unwrap_or_default())
            .bind(row.try_get::<String, _>("mods_path").unwrap_or_default())
            .bind(row.try_get::<String, _>("logicmods_path").unwrap_or_default())
            .bind(row.try_get::<String, _>("nativemods_path").unwrap_or_default())
            .bind(row.try_get::<String, _>("install_path").unwrap_or_default())
            .bind(row.try_get::<String, _>("steamcmd_path").unwrap_or_default())
            .bind(row.try_get::<Option<i64>, _>("pid").unwrap_or(None))
            .bind(row.try_get::<String, _>("launch_args").unwrap_or_default())
            .bind(row.try_get::<String, _>("workshop_dir").unwrap_or_default())
            .bind(row.try_get::<String, _>("server_name")?)
            .bind(row.try_get::<String, _>("server_description")?)
            .bind(row.try_get::<String, _>("server_password")?)
            .bind(row.try_get::<String, _>("admin_password")?)
            .bind(row.try_get::<i64, _>("max_players")?)
            .bind(env_vars.to_string())
            .bind(legacy_datetime_to_iso(row.try_get::<Option<String>, _>("created_at")?))
            .bind(legacy_datetime_to_iso(row.try_get::<Option<String>, _>("updated_at")?))
            .execute(pool)
            .await?;
            report.servers_imported += 1;
        }
    }

    let guard_value = serde_json::json!({
        "imported_at": crate::time::now_iso_naive_utc(),
        "source_path": legacy_db_path.to_string_lossy(),
        "backup_path": report.backup_path.to_string_lossy(),
        "presets": report.presets_imported,
        "ups_pals": report.ups_pals_imported,
        "ups_pals_skipped": report.ups_pals_skipped,
        "servers": report.servers_imported,
    });
    crate::meta::set(pool, LEGACY_IMPORT_META_KEY, &guard_value.to_string()).await?;

    Ok(Some(report))
}
