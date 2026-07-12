//! Static game-data handlers: forward data/json/* files, merged with the
//! l10n table for the current settings language. Each function is a
//! wire-exact port of the Python handler named in the plan table.

use serde_json::{json, Map, Value};

use psp_core::gamedata::GameData;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

/// Python reads app_state.settings.language; we read the settings row.
async fn current_language(ctx: &HandlerCtx<'_>) -> Result<String, HandlerError> {
    Ok(psp_db::settings::get_settings(&ctx.app.db).await?.language)
}

/// A data file as a JSON object; missing file behaves like Python's
/// auto-created empty {} file.
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

/// {id, localized_name, description, details} — shape shared by
/// active_skills / passive_skills / technologies.
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

/// active_skills_handler.py — {id, localized_name, description, details}.
pub async fn handle_get_active_skills(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let payload = skill_style_table(&ctx.app.game_data, &language, "active_skills");
    ctx.emitter.emit(MessageType::GetActiveSkills, &payload);
    Ok(())
}

/// passive_skills_handler.py — same shape as active_skills.
pub async fn handle_get_passive_skills(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let payload = skill_style_table(&ctx.app.game_data, &language, "passive_skills");
    ctx.emitter.emit(MessageType::GetPassiveSkills, &payload);
    Ok(())
}

/// technologies_handler.py — same shape as active_skills.
pub async fn handle_get_technologies(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let payload = skill_style_table(&ctx.app.game_data, &language, "technologies");
    ctx.emitter.emit(MessageType::GetTechnologies, &payload);
    Ok(())
}

/// elements_handler.py: {"localized_name": <l10n>, **details} — only
/// localized_name is taken from the l10n entry.
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

/// items_handler.py: {"id", "details", "info"} where info is the whole l10n entry.
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

/// missions_handler.py: quest_type defaults to "Main", rewards to {}.
pub async fn handle_get_missions(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let base = object_table(&ctx.app.game_data, "missions");
    let localization = object_table(&ctx.app.game_data, &format!("l10n/{language}/missions"));
    let mut merged = Map::new();
    for (mission_id, details) in base {
        let l10n_entry = localization.get(&mission_id);
        merged.insert(
            mission_id.clone(),
            json!({
                "id": mission_id,
                "localized_name": string_or(l10n_entry, "localized_name", &mission_id),
                "description": string_or(l10n_entry, "description", ""),
                "quest_type": details.get("quest_type").cloned()
                    .unwrap_or_else(|| Value::String("Main".into())),
                "rewards": details.get("rewards").cloned().unwrap_or_else(|| json!({})),
            }),
        );
    }
    ctx.emitter
        .emit(MessageType::GetMissions, &Value::Object(merged));
    Ok(())
}

/// buildings_handler.py: {"localized_name", "description", **details}.
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

/// work_suitability_handler.py — raw l10n/<lang>/work_suitability file.
pub async fn handle_get_work_suitability(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let payload = raw_file(
        &ctx.app.game_data,
        &format!("l10n/{language}/work_suitability"),
    );
    ctx.emitter.emit(MessageType::GetWorkSuitability, &payload);
    Ok(())
}

/// exp_handler.py — raw exp file.
pub async fn handle_get_exp_data(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "exp");
    ctx.emitter.emit(MessageType::GetExpData, &payload);
    Ok(())
}

/// friendship_handler.py — raw friendship file.
pub async fn handle_get_friendship_data(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "friendship");
    ctx.emitter.emit(MessageType::GetFriendshipData, &payload);
    Ok(())
}

/// map_objects_handler.py:get_map_objects_handler — raw map_objects file.
pub async fn handle_get_map_objects(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "map_objects");
    ctx.emitter.emit(MessageType::GetMapObjects, &payload);
    Ok(())
}

/// map_objects_handler.py:get_fast_travel_points_handler — raw fast_travel_points file.
pub async fn handle_get_fast_travel_points(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "fast_travel_points");
    ctx.emitter.emit(MessageType::GetFastTravelPoints, &payload);
    Ok(())
}

/// map_objects_handler.py:get_effigies_handler — raw effigies file.
pub async fn handle_get_effigies(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "effigies");
    ctx.emitter.emit(MessageType::GetEffigies, &payload);
    Ok(())
}

/// ui_common_handler.py:20 responds with MessageType.GET_ACTIVE_SKILLS —
/// a real Python bug that is now wire behavior. Do NOT fix it here.
pub async fn handle_get_ui_common(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let language = current_language(ctx).await?;
    let payload = raw_file(&ctx.app.game_data, &format!("l10n/{language}/ui"));
    ctx.emitter.emit(MessageType::GetActiveSkills, &payload);
    Ok(())
}

/// version_handler.py — the crate version string.
pub async fn handle_get_version(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let version = ctx.app.game_data.version().to_string();
    ctx.emitter.emit(MessageType::GetVersion, &version);
    Ok(())
}

/// pal_handler.py:26-50 — mutates the base entry, appending localized_name
/// and description.
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

/// lab_research_handler.py:16-35 — fallback description is null (Python None).
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
            r#"{"M1": {"quest_type": "Side", "rewards": {"gold": 5}}, "M2": {}}"#,
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
        // Handlers with no dedicated table logic to exercise beyond correct
        // wiring (right GameData key, right response MessageType): still
        // given distinct fixture content per-file so a copy-paste mistake
        // that reads the wrong file or emits the wrong type is caught.
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
        fs::write(json_dir.join("effigies.json"), r#"{"Eff1": {"x": 2}}"#).unwrap();
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
                   "quest_type": "Side", "rewards": {"gold": 5}})
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

        // Missing file → empty object (Python auto-creates an empty {} file).
        let frame = run_handler!(test, handle_get_map_objects);
        assert_eq!(frame["type"], "get_map_objects");
        assert_eq!(frame["data"], json!({}));
    }

    #[tokio::test]
    async fn remaining_raw_forwarders_send_correct_file_and_type() {
        // work_suitability, friendship_data, fast_travel_points and effigies
        // share the same raw_file() plumbing as exp_data/map_objects but have
        // no dedicated coverage in the brief's test list; pin each one's
        // GameData key and response MessageType explicitly so a copy-paste
        // mistake (e.g. friendship_data reading fast_travel_points.json) is
        // still catchable.
        let mut test = TestContext::new(write_fixture_tree).await;

        let frame = run_handler!(test, handle_get_work_suitability);
        assert_eq!(frame["type"], "get_work_suitability");
        assert_eq!(frame["data"], json!({"Kindling": "Kindling Work"}));

        let frame = run_handler!(test, handle_get_friendship_data);
        assert_eq!(frame["type"], "get_friendship_data");
        assert_eq!(frame["data"], json!({"1": {"NextFriendshipPoint": 100}}));

        let frame = run_handler!(test, handle_get_fast_travel_points);
        assert_eq!(frame["type"], "get_fast_travel_points");
        assert_eq!(frame["data"], json!({"FT1": {"x": 1}}));

        let frame = run_handler!(test, handle_get_effigies);
        assert_eq!(frame["type"], "get_effigies");
        assert_eq!(frame["data"], json!({"Eff1": {"x": 2}}));
    }

    #[tokio::test]
    async fn ui_common_response_is_typed_get_active_skills() {
        // Wire-exact reproduction of the Python bug in ui_common_handler.py:20.
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
