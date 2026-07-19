//! Static game-data handlers: forward data/json/* files, merged with the
//! l10n table for the current settings language.

use serde_json::{json, Map, Value};

use psp_core::gamedata::GameData;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

async fn current_language(ctx: &HandlerCtx<'_>) -> Result<String, HandlerError> {
    Ok(psp_db::settings::get_settings(&ctx.app.db).await?.language)
}

fn object_table(game_data: &GameData, key: &str) -> Map<String, Value> {
    game_data
        .get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn raw_file(game_data: &GameData, key: &str) -> Value {
    game_data.get(key).cloned().unwrap_or_else(|| json!({}))
}

fn string_or(entry: Option<&Value>, field: &str, fallback: &str) -> Value {
    entry
        .and_then(|value| value.get(field))
        .cloned()
        .unwrap_or_else(|| Value::String(fallback.to_string()))
}

/// Wire shape `{id, localized_name, description, details}`, shared by the
/// active_skills / passive_skills / technologies responses.
fn skill_style_table(game_data: &GameData, language: &str, file: &str) -> Value {
    let base = object_table(game_data, file);
    let localization = object_table(game_data, &format!("l10n/{language}/{file}"));
    let mut merged = Map::new();
    for (entry_id, details) in base {
        let l10n_entry = localization.get(&entry_id);
        merged.insert(
            entry_id.clone(),
            json!({
                "id": entry_id,
                "localized_name": string_or(l10n_entry, "localized_name", &entry_id),
                "description": string_or(l10n_entry, "description", ""),
                "details": details,
            }),
        );
    }
    Value::Object(merged)
}

pub async fn handle_get_active_skills(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let payload = skill_style_table(&ctx.app.game_data, &language, "active_skills");
    ctx.emitter.emit(MessageType::GetActiveSkills, &payload);
    Ok(())
}

pub async fn handle_get_passive_skills(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let payload = skill_style_table(&ctx.app.game_data, &language, "passive_skills");
    ctx.emitter.emit(MessageType::GetPassiveSkills, &payload);
    Ok(())
}

pub async fn handle_get_technologies(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let payload = skill_style_table(&ctx.app.game_data, &language, "technologies");
    ctx.emitter.emit(MessageType::GetTechnologies, &payload);
    Ok(())
}

/// Wire shape `{localized_name, **details}` — only `localized_name` comes
/// from the l10n entry; every other field is spread from the base entry.
pub async fn handle_get_elements(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let base = object_table(&ctx.app.game_data, "elements");
    let localization = object_table(&ctx.app.game_data, &format!("l10n/{language}/elements"));
    let mut merged = Map::new();
    for (element_id, details) in base {
        let mut entry = Map::new();
        entry.insert(
            "localized_name".into(),
            string_or(localization.get(&element_id), "localized_name", &element_id),
        );
        if let Some(detail_fields) = details.as_object() {
            for (field, value) in detail_fields {
                entry.insert(field.clone(), value.clone());
            }
        }
        merged.insert(element_id, Value::Object(entry));
    }
    ctx.emitter
        .emit(MessageType::GetElements, &Value::Object(merged));
    Ok(())
}

/// Wire shape `{id, details, info}`, where `info` is the whole l10n entry.
pub async fn handle_get_items(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let base = object_table(&ctx.app.game_data, "items");
    let localization = object_table(&ctx.app.game_data, &format!("l10n/{language}/items"));
    let mut merged = Map::new();
    for (item_id, details) in base {
        let info = localization
            .get(&item_id)
            .cloned()
            .unwrap_or_else(|| json!({"localized_name": item_id, "description": ""}));
        merged.insert(
            item_id.clone(),
            json!({"id": item_id, "details": details, "info": info}),
        );
    }
    ctx.emitter
        .emit(MessageType::GetItems, &Value::Object(merged));
    Ok(())
}

/// `quest_type` and `rewards` are always present on the wire, defaulting to
/// "Main" / `{}` so the frontend never has to null-check them.
pub async fn handle_get_missions(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let base = object_table(&ctx.app.game_data, "missions");
    let localization = object_table(&ctx.app.game_data, &format!("l10n/{language}/missions"));
    let mut merged = Map::new();
    for (mission_id, details) in base {
        let l10n_entry = localization.get(&mission_id);
        let quest_type = details
            .get("quest_type")
            .and_then(Value::as_str)
            .map(|raw| raw.strip_prefix("EPalQuestType::").unwrap_or(raw))
            .unwrap_or("Main");
        merged.insert(
            mission_id.clone(),
            json!({
                "id": mission_id,
                "localized_name": string_or(l10n_entry, "localized_name", &mission_id),
                "description": string_or(l10n_entry, "description", ""),
                "quest_type": quest_type,
                "rewards": details.get("rewards").cloned().unwrap_or_else(|| json!({})),
            }),
        );
    }
    ctx.emitter
        .emit(MessageType::GetMissions, &Value::Object(merged));
    Ok(())
}

/// Wire shape `{localized_name, description, **details}`.
pub async fn handle_get_buildings(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let base = object_table(&ctx.app.game_data, "buildings");
    let localization = object_table(&ctx.app.game_data, &format!("l10n/{language}/buildings"));
    let mut merged = Map::new();
    for (building_id, details) in base {
        let l10n_entry = localization.get(&building_id);
        let mut entry = Map::new();
        entry.insert(
            "localized_name".into(),
            string_or(l10n_entry, "localized_name", &building_id),
        );
        entry.insert(
            "description".into(),
            string_or(l10n_entry, "description", ""),
        );
        if let Some(detail_fields) = details.as_object() {
            for (field, value) in detail_fields {
                entry.insert(field.clone(), value.clone());
            }
        }
        merged.insert(building_id, Value::Object(entry));
    }
    ctx.emitter
        .emit(MessageType::GetBuildings, &Value::Object(merged));
    Ok(())
}

pub async fn handle_get_work_suitability(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let payload = raw_file(
        &ctx.app.game_data,
        &format!("l10n/{language}/work_suitability"),
    );
    ctx.emitter.emit(MessageType::GetWorkSuitability, &payload);
    Ok(())
}

pub async fn handle_get_exp_data(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "exp");
    ctx.emitter.emit(MessageType::GetExpData, &payload);
    Ok(())
}

/// Localization is merged INTO the base entry (rather than nested under a sub-object),
/// so every relic on the wire carries `localized_name` and `description` alongside its
/// rank table. Same shape as `handle_get_pals`.
pub async fn handle_get_relic_data(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let base = object_table(&ctx.app.game_data, "relic_data");
    let localization = object_table(&ctx.app.game_data, &format!("l10n/{language}/relics"));
    let mut merged = Map::new();
    for (relic_key, mut entry_value) in base {
        let entry = entry_value.as_object_mut().ok_or_else(|| {
            HandlerError::Other(format!(
                "relic_data.json entry {relic_key} is not an object"
            ))
        })?;
        let l10n_entry = localization.get(&relic_key);
        entry.insert(
            "localized_name".into(),
            string_or(l10n_entry, "localized_name", &relic_key),
        );
        entry.insert(
            "description".into(),
            string_or(l10n_entry, "description", "No description available"),
        );
        merged.insert(relic_key, entry_value);
    }
    ctx.emitter
        .emit(MessageType::GetRelicData, &Value::Object(merged));
    Ok(())
}

pub async fn handle_get_friendship_data(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "friendship");
    ctx.emitter.emit(MessageType::GetFriendshipData, &payload);
    Ok(())
}

pub async fn handle_get_map_objects(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "map_objects");
    ctx.emitter.emit(MessageType::GetMapObjects, &payload);
    Ok(())
}

pub async fn handle_get_bosses(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "bosses");
    ctx.emitter.emit(MessageType::GetBosses, &payload);
    Ok(())
}

/// All 405 collectible relics of every `EPalRelicType`; `effigies` is the
/// CapturePower subset of this same table.
pub async fn handle_get_relics(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "relics");
    ctx.emitter.emit(MessageType::GetRelics, &payload);
    Ok(())
}

/// Localization merged INTO each base entry (same shape as `handle_get_relic_data`),
/// so every point on the wire keeps `class`/coords/`id` and carries `localized_name`.
/// Watchtowers (`BP_LevelObject_UnlockMapPoint_C`) flow through unchanged — the
/// client branches on `class`.
pub async fn handle_get_fast_travel_points(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let base = object_table(&ctx.app.game_data, "fast_travel_points");
    let localization =
        object_table(&ctx.app.game_data, &format!("l10n/{language}/fast_travel_points"));
    let mut merged = Map::new();
    for (guid, mut entry_value) in base {
        let entry = entry_value.as_object_mut().ok_or_else(|| {
            HandlerError::Other(format!("fast_travel_points.json entry {guid} is not an object"))
        })?;
        let l10n_entry = localization.get(&guid);
        entry.insert(
            "localized_name".into(),
            string_or(l10n_entry, "localized_name", &guid),
        );
        merged.insert(guid, entry_value);
    }
    ctx.emitter
        .emit(MessageType::GetFastTravelPoints, &Value::Object(merged));
    Ok(())
}

pub async fn handle_get_effigies(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "effigies");
    ctx.emitter.emit(MessageType::GetEffigies, &payload);
    Ok(())
}

/// Responds under the `get_active_skills` message type, NOT `get_ui_common`.
/// The frontend correlates on that type — do not "fix" it here.
pub async fn handle_get_ui_common(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let payload = raw_file(&ctx.app.game_data, &format!("l10n/{language}/ui"));
    ctx.emitter.emit(MessageType::GetActiveSkills, &payload);
    Ok(())
}

pub async fn handle_get_version(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let version = ctx.app.game_data.version().to_string();
    ctx.emitter.emit(MessageType::GetVersion, &version);
    Ok(())
}

/// Localization is merged INTO the base entry (rather than nested under a
/// sub-object), so every pal on the wire carries `localized_name` and
/// `description` alongside its base fields.
pub async fn handle_get_pals(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let base = object_table(&ctx.app.game_data, "pals");
    let localization = object_table(&ctx.app.game_data, &format!("l10n/{language}/pals"));
    let mut merged = Map::new();
    for (code_name, mut pal_info) in base {
        let entry = pal_info.as_object_mut().ok_or_else(|| {
            HandlerError::Other(format!("pals.json entry {code_name} is not an object"))
        })?;
        let l10n_entry = localization.get(&code_name);
        if l10n_entry.is_some() {
            entry.insert(
                "localized_name".into(),
                string_or(l10n_entry, "localized_name", &code_name),
            );
            entry.insert(
                "description".into(),
                string_or(l10n_entry, "description", "No description available"),
            );
        } else {
            entry.insert("localized_name".into(), Value::String(code_name.clone()));
            entry.insert(
                "description".into(),
                Value::String("No description available".into()),
            );
        }
        merged.insert(code_name, pal_info);
    }
    ctx.emitter
        .emit(MessageType::GetPals, &Value::Object(merged));
    Ok(())
}

/// `description` is present but `null` when the l10n table has no entry — the
/// frontend distinguishes null from an empty string here.
pub async fn handle_get_lab_research(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let base = object_table(&ctx.app.game_data, "lab_research");
    let localization = object_table(&ctx.app.game_data, &format!("l10n/{language}/lab_research"));
    let mut merged = Map::new();
    for (research_id, details) in base {
        let l10n_entry = localization.get(&research_id);
        let description = l10n_entry
            .and_then(|entry| entry.get("description"))
            .cloned()
            .unwrap_or(Value::Null);
        merged.insert(
            research_id.clone(),
            json!({
                "id": research_id,
                "localized_name": string_or(l10n_entry, "localized_name", &research_id),
                "description": description,
                "details": details,
            }),
        );
    }
    ctx.emitter
        .emit(MessageType::GetLabResearch, &Value::Object(merged));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::HandlerCtx;
    use crate::test_support::TestContext;
    use serde_json::json;
    use std::fs;

    /// Builds a tiny data/json tree exercising every merge rule.
    fn write_fixture_tree(json_dir: &std::path::Path) {
        fs::create_dir_all(json_dir.join("l10n/en")).unwrap();
        fs::write(
            json_dir.join("active_skills.json"),
            r#"{"Fireball": {"power": 30}, "NoL10n": {"power": 1}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("l10n/en/active_skills.json"),
            r#"{"Fireball": {"localized_name": "Ignis Blast", "description": "burns"}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("elements.json"),
            r#"{"Fire": {"badge_icon": "f.png"}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("l10n/en/elements.json"),
            r#"{"Fire": {"localized_name": "Feuer"}}"#,
        )
        .unwrap();
        fs::write(json_dir.join("items.json"), r#"{"Sword": {"tier": 2}}"#).unwrap();
        fs::write(
            json_dir.join("l10n/en/items.json"),
            r#"{"Sword": {"localized_name": "Schwert", "description": "stabby"}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("missions.json"),
            r#"{"M1": {"quest_type": "EPalQuestType::Sub", "rewards": {"gold": 5}}, "M2": {}}"#,
        )
        .unwrap();
        fs::write(json_dir.join("l10n/en/missions.json"), r#"{}"#).unwrap();
        fs::write(
            json_dir.join("buildings.json"),
            r#"{"Bench": {"category": "work"}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("l10n/en/buildings.json"),
            r#"{"Bench": {"localized_name": "Werkbank", "description": "craft"}}"#,
        )
        .unwrap();
        fs::write(json_dir.join("exp.json"), r#"{"1": {"TotalEXP": 0}}"#).unwrap();
        fs::write(json_dir.join("l10n/en/ui.json"), r#"{"health": "Health"}"#).unwrap();
        fs::write(
            json_dir.join("pals.json"),
            r#"{"PinkCat": {"code_name": "PinkCat"}, "Mystery": {"code_name": "Mystery"}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("l10n/en/pals.json"),
            r#"{"PinkCat": {"localized_name": "Cattiva", "description": "cheeky"}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("lab_research.json"),
            r#"{"R1": {"cost": 100}}"#,
        )
        .unwrap();
        fs::write(json_dir.join("l10n/en/lab_research.json"), r#"{}"#).unwrap();
        // Distinct content per file, so a handler that reads the wrong file
        // (or emits the wrong response type) is caught.
        fs::write(
            json_dir.join("passive_skills.json"),
            r#"{"Vampiric": {"power": 5}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("l10n/en/passive_skills.json"),
            r#"{"Vampiric": {"localized_name": "Vampirism", "description": "drains"}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("technologies.json"),
            r#"{"Bow": {"tier": 1}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("l10n/en/technologies.json"),
            r#"{"Bow": {"localized_name": "Bogen", "description": "shoots"}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("l10n/en/work_suitability.json"),
            r#"{"Kindling": "Kindling Work"}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("friendship.json"),
            r#"{"1": {"NextFriendshipPoint": 100}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("fast_travel_points.json"),
            r#"{"FT1": {"x": 1}}"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("l10n/en/fast_travel_points.json"),
            r#"{"FT1": {"localized_name": "Beach"}}"#,
        )
        .unwrap();
        fs::write(json_dir.join("effigies.json"), r#"{"Eff1": {"x": 2}}"#).unwrap();
        fs::write(
            json_dir.join("relics.json"),
            r#"{"Rel1": {"x": 3, "relic_type": "jump_power"}}"#,
        )
        .unwrap();
    }

    macro_rules! run_handler {
        ($test:ident, $handler:ident) => {{
            let mut ctx = HandlerCtx {
                session: &mut $test.session,
                app: &$test.app,
                emitter: &$test.emitter,
                attachment: None,
            };
            $handler(&mut ctx).await.unwrap();
            $test.next_frame_json()
        }};
    }

    #[tokio::test]
    async fn active_skills_merge_with_l10n_fallback() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_active_skills);
        assert_eq!(frame["type"], "get_active_skills");
        assert_eq!(
            frame["data"]["Fireball"],
            json!({"id": "Fireball", "localized_name": "Ignis Blast",
                   "description": "burns", "details": {"power": 30}})
        );
        assert_eq!(
            frame["data"]["NoL10n"],
            json!({"id": "NoL10n", "localized_name": "NoL10n",
                   "description": "", "details": {"power": 1}})
        );
    }

    #[tokio::test]
    async fn elements_take_only_localized_name_from_l10n() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_elements);
        assert_eq!(frame["type"], "get_elements");
        assert_eq!(
            frame["data"]["Fire"],
            json!({"localized_name": "Feuer", "badge_icon": "f.png"})
        );
    }

    #[tokio::test]
    async fn passive_skills_merge_with_l10n() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_passive_skills);
        assert_eq!(frame["type"], "get_passive_skills");
        assert_eq!(
            frame["data"]["Vampiric"],
            json!({"id": "Vampiric", "localized_name": "Vampirism",
                   "description": "drains", "details": {"power": 5}})
        );
    }

    #[tokio::test]
    async fn technologies_merge_with_l10n() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_technologies);
        assert_eq!(frame["type"], "get_technologies");
        assert_eq!(
            frame["data"]["Bow"],
            json!({"id": "Bow", "localized_name": "Bogen",
                   "description": "shoots", "details": {"tier": 1}})
        );
    }

    #[tokio::test]
    async fn items_wrap_details_and_info() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_items);
        assert_eq!(frame["type"], "get_items");
        assert_eq!(
            frame["data"]["Sword"],
            json!({"id": "Sword", "details": {"tier": 2},
                   "info": {"localized_name": "Schwert", "description": "stabby"}})
        );
    }

    #[tokio::test]
    async fn missions_default_quest_type_and_rewards() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_missions);
        assert_eq!(frame["type"], "get_missions");
        assert_eq!(
            frame["data"]["M1"],
            json!({"id": "M1", "localized_name": "M1", "description": "",
                   "quest_type": "Sub", "rewards": {"gold": 5}})
        );
        assert_eq!(
            frame["data"]["M2"],
            json!({"id": "M2", "localized_name": "M2", "description": "",
                   "quest_type": "Main", "rewards": {}})
        );
    }

    #[tokio::test]
    async fn buildings_flatten_details() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_buildings);
        assert_eq!(frame["type"], "get_buildings");
        assert_eq!(
            frame["data"]["Bench"],
            json!({"localized_name": "Werkbank", "description": "craft", "category": "work"})
        );
    }

    #[tokio::test]
    async fn raw_forwarders_send_files_verbatim() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_exp_data);
        assert_eq!(frame["type"], "get_exp_data");
        assert_eq!(frame["data"], json!({"1": {"TotalEXP": 0}}));

        // Missing file → empty object.
        let frame = run_handler!(test, handle_get_map_objects);
        assert_eq!(frame["type"], "get_map_objects");
        assert_eq!(frame["data"], json!({}));
    }

    #[tokio::test]
    async fn get_bosses_returns_the_raw_file() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_bosses);
        assert_eq!(frame["type"], "get_bosses");
        assert!(frame["data"].is_object(), "bosses payload must be an object");
    }

    #[tokio::test]
    async fn get_relics_returns_the_raw_file() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_relics);
        assert_eq!(frame["type"], "get_relics");
        assert_eq!(
            frame["data"]["Rel1"],
            json!({"x": 3, "relic_type": "jump_power"})
        );
    }

    #[tokio::test]
    async fn remaining_raw_forwarders_send_correct_file_and_type() {
        // These three share raw_file()'s plumbing; pin each one's GameData key
        // and response type so a mix-up (e.g. friendship_data reading
        // fast_travel_points.json) is caught.
        let mut test = TestContext::new(write_fixture_tree).await;

        let frame = run_handler!(test, handle_get_work_suitability);
        assert_eq!(frame["type"], "get_work_suitability");
        assert_eq!(frame["data"], json!({"Kindling": "Kindling Work"}));

        let frame = run_handler!(test, handle_get_friendship_data);
        assert_eq!(frame["type"], "get_friendship_data");
        assert_eq!(frame["data"], json!({"1": {"NextFriendshipPoint": 100}}));

        let frame = run_handler!(test, handle_get_effigies);
        assert_eq!(frame["type"], "get_effigies");
        assert_eq!(frame["data"], json!({"Eff1": {"x": 2}}));
    }

    #[tokio::test]
    async fn fast_travel_points_merge_with_l10n_and_keep_class() {
        // Unlike the raw forwarders above, fast_travel_points merges l10n
        // INTO the base entry (same shape as handle_get_relic_data) while
        // preserving every base field.
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_fast_travel_points);
        assert_eq!(frame["type"], "get_fast_travel_points");
        assert_eq!(
            frame["data"]["FT1"],
            json!({"x": 1, "localized_name": "Beach"})
        );
    }

    #[tokio::test]
    async fn ui_common_response_is_typed_get_active_skills() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_ui_common);
        assert_eq!(frame["type"], "get_active_skills");
        assert_eq!(frame["data"], json!({"health": "Health"}));
    }

    #[tokio::test]
    async fn version_reports_cargo_package_version() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_version);
        assert_eq!(frame["type"], "get_version");
        assert_eq!(frame["data"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn pals_append_localization_into_base_entries() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_pals);
        assert_eq!(frame["type"], "get_pals");
        assert_eq!(
            frame["data"]["PinkCat"],
            json!({"code_name": "PinkCat", "localized_name": "Cattiva", "description": "cheeky"})
        );
        assert_eq!(
            frame["data"]["Mystery"],
            json!({"code_name": "Mystery", "localized_name": "Mystery",
                   "description": "No description available"})
        );
    }

    #[tokio::test]
    async fn lab_research_fallback_description_is_null() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_lab_research);
        assert_eq!(frame["type"], "get_lab_research");
        assert_eq!(
            frame["data"]["R1"],
            json!({"id": "R1", "localized_name": "R1", "description": null,
                   "details": {"cost": 100}})
        );
    }
}
