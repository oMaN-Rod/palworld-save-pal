//! Static game-data handlers: forward data/json/* files, merged with the
//! l10n table for the current settings language.

use serde_json::{json, Map, Value};

use psp_core::gamedata::GameData;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

async fn current_language(ctx: &HandlerCtx<'_>) -> Result<String, HandlerError> {
    Ok(psp_db::settings::get_settings(&*ctx.app.driver)
        .await?
        .language)
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

pub async fn handle_get_map_object_footprints(
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "map_object_footprints");
    ctx.emitter
        .emit(MessageType::GetMapObjectFootprints, &payload);
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GetBaseStructuresData {
    pub base_id: uuid::Uuid,
}

/// Read-only placed-structure geometry for one base. EVERY outcome answers
/// under `get_base_structures` so the frontend's request/response correlation
/// resolves — a malformed or missing `base_id` is a soft `{"error": ...}`
/// payload (omitting `base_id`, since none could be parsed), same convention
/// as the no-save-loaded branch below, never the dispatcher's hard `error`
/// frame nor an empty list the map would render as "this base has nothing
/// built in it".
pub async fn handle_get_base_structures(
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let request: GetBaseStructuresData = match serde_json::from_value(data) {
        Ok(request) => request,
        Err(error) => {
            ctx.emitter.emit(
                MessageType::GetBaseStructures,
                &json!({"error": format!("Invalid base_structures request: {error}")}),
            );
            return Ok(());
        }
    };
    let Some(session) = ctx.session.save.as_ref() else {
        ctx.emitter.emit(
            MessageType::GetBaseStructures,
            &json!({"base_id": request.base_id, "error": "No save file loaded"}),
        );
        return Ok(());
    };
    let structures = psp_core::domain::guild::base_structures(session, request.base_id);
    ctx.emitter.emit(
        MessageType::GetBaseStructures,
        &json!({"base_id": request.base_id, "structures": structures}),
    );
    Ok(())
}

pub async fn handle_get_bosses(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "bosses");
    ctx.emitter.emit(MessageType::GetBosses, &payload);
    Ok(())
}

pub async fn handle_get_dungeons(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let payload = raw_file(&ctx.app.game_data, "dungeons");
    ctx.emitter.emit(MessageType::GetDungeons, &payload);
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
    let localization = object_table(
        &ctx.app.game_data,
        &format!("l10n/{language}/fast_travel_points"),
    );
    let mut merged = Map::new();
    for (guid, mut entry_value) in base {
        let entry = entry_value.as_object_mut().ok_or_else(|| {
            HandlerError::Other(format!(
                "fast_travel_points.json entry {guid} is not an object"
            ))
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

/// The marker-layer artifacts `get_map_layer` will serve. A request names a
/// file to read off disk, so an id outside this list is refused rather than
/// resolved as a path.
const MAP_LAYERS: [&str; 13] = [
    "fast_travel_points",
    "dungeons",
    "bosses",
    "relics",
    "effigies",
    "towers",
    "notes",
    "eggs_spawners",
    "chests",
    "camps",
    "ancient_ruins",
    "kinship_peach",
    "skill_fruits",
];

#[derive(Debug, serde::Deserialize)]
pub struct GetMapLayerData {
    pub layers: Vec<String>,
}

/// Folds `localized_name` onto each entry of a marker artifact from
/// `l10n/{language}/{artifact}`, the same shape `handle_get_fast_travel_points`
/// produces. Driven by the l10n file's existence, so an artifact gains names the
/// moment its table ships — no list to keep in step.
///
/// Three cases serve the artifact untouched instead:
/// - no l10n table for this artifact, or none for this language;
/// - a top-level ARRAY (`eggs_spawners`, `camps`), whose entries carry no ids to
///   key a merge on — inventing one would tie the wire shape to array order;
/// - an entry the l10n table does not cover. Unlike the per-artifact handlers,
///   which fall back to the entry's own key as its display name, this path can
///   be pointed at a table that was never meant for it: `l10n/{lang}/relics.json`
///   localizes `relic_data.json`'s 13 relic TYPES, while the `relics` artifact is
///   407 markers keyed by instance id — zero keys in common. A blanket fallback
///   would hand every one of those markers its own GUID as a name.
fn localized_map_layer(game_data: &GameData, language: &str, artifact: &str) -> Value {
    let raw = raw_file(game_data, artifact);
    let Some(localization) = game_data
        .get(&format!("l10n/{language}/{artifact}"))
        .and_then(Value::as_object)
    else {
        return raw;
    };
    let Value::Object(base) = raw else {
        return raw;
    };
    let mut merged = Map::new();
    for (entry_id, mut entry_value) in base {
        let localized_name = localization
            .get(&entry_id)
            .and_then(|l10n_entry| l10n_entry.get("localized_name"));
        if let (Some(entry), Some(localized_name)) = (entry_value.as_object_mut(), localized_name) {
            entry.insert("localized_name".into(), localized_name.clone());
        }
        merged.insert(entry_id, entry_value);
    }
    Value::Object(merged)
}

/// Marker-layer artifacts for the requested ids, keyed by id under `layers`,
/// each localized by `localized_map_layer`. Batched — one request carrying N
/// ids, never N requests — because the frontend correlates responses by message
/// TYPE alone, so two single-layer requests in flight would resolve against
/// each other.
///
/// `fast_travel_points` comes out of here in the same shape
/// `get_fast_travel_points` produces, so a layer served through either message
/// keeps its names.
///
/// EVERY outcome answers under `get_map_layer`, refusals included, so a client
/// waiting on that type always resolves. An unrecognized id fails the whole
/// request rather than dropping its key: a key the client asked for and never
/// received would leave it waiting forever.
pub async fn handle_get_map_layer(
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let request: GetMapLayerData = match serde_json::from_value(data) {
        Ok(request) => request,
        Err(error) => {
            ctx.emitter.emit(
                MessageType::GetMapLayer,
                &json!({"error": format!("Invalid map_layer request: {error}")}),
            );
            return Ok(());
        }
    };
    if request.layers.is_empty() {
        ctx.emitter.emit(
            MessageType::GetMapLayer,
            &json!({"error": "No map layers requested"}),
        );
        return Ok(());
    }
    // Every id is checked before anything is read, so a refusal never depends
    // on the settings lookup below.
    if let Some(unknown) = request
        .layers
        .iter()
        .find(|layer_id| !MAP_LAYERS.contains(&layer_id.as_str()))
    {
        ctx.emitter.emit(
            MessageType::GetMapLayer,
            &json!({"error": format!("Unknown map layer: {unknown}")}),
        );
        return Ok(());
    }
    let language = current_language(ctx).await?;
    let mut layers = Map::new();
    for layer_id in request.layers {
        let payload = localized_map_layer(&ctx.app.game_data, &language, &layer_id);
        layers.insert(layer_id, payload);
    }
    ctx.emitter
        .emit(MessageType::GetMapLayer, &json!({"layers": layers}));
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
        fs::write(
            json_dir.join("map_object_footprints.json"),
            r#"{"PalBoxV2": {"sx": 500, "sy": 400, "sz": 390, "ox": -150, "oy": 0, "oz": 200, "typeA": "Other"}}"#,
        )
        .unwrap();
        // Marker layers. Some real artifacts are top-level arrays rather than
        // objects, so the fixtures keep both shapes.
        fs::write(
            json_dir.join("towers.json"),
            r#"{"Tower1": {"class": "BP_PalBossTower_C", "x": 7}}"#,
        )
        .unwrap();
        fs::write(json_dir.join("notes.json"), r#"{"Day0": {"x": 8}}"#).unwrap();
        fs::write(
            json_dir.join("eggs_spawners.json"),
            r#"[{"class": "spawner", "x": 9}]"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("chests.json"),
            r#"[{"class": "chest", "x": 10}]"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("camps.json"),
            r#"[{"class": "camp", "x": 11}]"#,
        )
        .unwrap();
        fs::write(
            json_dir.join("l10n/en/towers.json"),
            r#"{"Tower1": {"localized_name": "Rayne Syndicate Tower"}}"#,
        )
        .unwrap();
        // Keyed by relic TYPE, while relics.json is keyed by marker instance
        // id — the two share no keys at all, exactly as on disk.
        fs::write(
            json_dir.join("l10n/en/relics.json"),
            r#"{"jump_power": {"localized_name": "Jump Power"}}"#,
        )
        .unwrap();
        // An l10n table alongside an array-shaped artifact, which has no entry
        // ids to key a merge on.
        fs::write(
            json_dir.join("l10n/en/camps.json"),
            r#"{"0": {"localized_name": "First Camp"}}"#,
        )
        .unwrap();
    }

    macro_rules! run_handler {
        ($test:ident, $handler:ident) => {{
            let mut ctx = HandlerCtx {
                session: &mut $test.session,
                app: &$test.app,
                emitter: &$test.emitter,
                blueprints: &mut $test.blueprints,
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

    /// `run_handler!`'s counterpart for the handlers that take a request
    /// payload; yields the `Result` so the error paths stay assertable.
    macro_rules! run_handler_with_data {
        ($test:ident, $handler:ident, $data:expr) => {{
            let mut ctx = HandlerCtx {
                session: &mut $test.session,
                app: &$test.app,
                emitter: &$test.emitter,
                blueprints: &mut $test.blueprints,
                attachment: None,
            };
            $handler($data, &mut ctx).await
        }};
    }

    const FIXTURE_BASE_ID: &str = "11111111-1111-1111-1111-111111111111";
    const FIXTURE_OTHER_BASE_ID: &str = "22222222-2222-2222-2222-222222222222";
    const FIXTURE_INSTANCE_ID: &str = "33333333-3333-3333-3333-333333333333";
    const FIXTURE_BUILDER_ID: &str = "44444444-4444-4444-4444-444444444444";

    /// A world holding exactly one `MapObjectSaveData` element, a `PalBoxV2`
    /// belonging to `FIXTURE_BASE_ID`.
    fn level_with_one_structure() -> psp_core::ue::Save {
        use psp_core::ue::games::palworld::{
            PalMapModel, PalMapObjectHp, PalStageInstanceId, PalTransform,
        };
        use psp_core::ue::{
            Double, FGuid, Header, PackageVersion, Properties, Property, PropertySchemas, Quat,
            Root, Save, StructValue, ValueVec, Vector,
        };

        fn guid(text: &str) -> FGuid {
            serde_json::from_value(Value::String(text.to_string())).unwrap()
        }

        let model = PalMapModel {
            instance_id: guid(FIXTURE_INSTANCE_ID),
            concrete_model_instance_id: FGuid::nil(),
            base_camp_id_belong_to: guid(FIXTURE_BASE_ID),
            group_id_belong_to: FGuid::nil(),
            hp: PalMapObjectHp {
                current: 40,
                max: 100,
            },
            initial_transform_cache: PalTransform {
                rotation: Quat {
                    x: Double(0.0),
                    y: Double(0.0),
                    z: Double(0.0),
                    w: Double(1.0),
                },
                translation: Vector {
                    x: Double(10.0),
                    y: Double(20.0),
                    z: Double(30.0),
                },
                scale: Vector {
                    x: Double(2.0),
                    y: Double(3.0),
                    z: Double(4.0),
                },
            },
            repair_work_id: FGuid::nil(),
            owner_spawner_level_object_instance_id: FGuid::nil(),
            owner_instance_id: FGuid::nil(),
            build_player_uid: guid(FIXTURE_BUILDER_ID),
            interact_restrict_type: 0,
            deterioration_damage: 0.0,
            stage_instance_id_belong_to: PalStageInstanceId {
                id: FGuid::nil(),
                valid: 0,
            },
            unknown_bytes: vec![],
        };

        let mut model_props = Properties::default();
        model_props.insert(
            "RawData",
            Property::Struct(StructValue::Game(psp_core::ue::PalStruct::MapModel(
                Box::new(model),
            ))),
        );
        let mut object_props = Properties::default();
        object_props.insert("MapObjectId", Property::Name("PalBoxV2".to_string()));
        object_props.insert("Model", Property::Struct(StructValue::Struct(model_props)));

        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "MapObjectSaveData",
            Property::Array(ValueVec::Struct(vec![StructValue::Struct(object_props)])),
        );
        let mut root_properties = Properties::default();
        root_properties.insert(
            "worldSaveData",
            Property::Struct(StructValue::Struct(world_save_data)),
        );

        Save {
            header: Header {
                magic: 0,
                save_game_version: 0,
                package_version: PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: PropertySchemas::default(),
            root: Root {
                save_game_type: String::new(),
                properties: root_properties,
            },
            extra: Vec::new(),
        }
    }

    fn test_with_one_structure(test: &mut TestContext) {
        test.session.save = Some(psp_core::session::SaveSession::new_for_tests(
            psp_core::session::SaveKind::InMemory,
            level_with_one_structure(),
        ));
    }

    #[tokio::test]
    async fn map_object_footprints_pass_through_unchanged() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_map_object_footprints);
        assert_eq!(frame["type"], "get_map_object_footprints");
        assert_eq!(frame["data"]["PalBoxV2"]["sx"], 500);
        assert_eq!(frame["data"]["PalBoxV2"]["typeA"], "Other");
    }

    #[tokio::test]
    async fn base_structures_emit_the_requested_base_and_its_structures() {
        let mut test = TestContext::new(write_fixture_tree).await;
        test_with_one_structure(&mut test);

        run_handler_with_data!(
            test,
            handle_get_base_structures,
            json!({"base_id": FIXTURE_BASE_ID})
        )
        .unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_base_structures");
        assert_eq!(frame["data"]["base_id"], FIXTURE_BASE_ID);
        assert_eq!(
            frame["data"]["structures"],
            json!([{
                "instance_id": FIXTURE_INSTANCE_ID,
                "map_object_id": "PalBoxV2",
                "x": 10.0,
                "y": 20.0,
                "z": 30.0,
                "yaw": 0.0,
                "scale_x": 2.0,
                "scale_y": 3.0,
                "scale_z": 4.0,
                "hp_current": 40,
                "hp_max": 100,
                "build_player_uid": FIXTURE_BUILDER_ID,
            }])
        );
    }

    /// The `base_id` is honoured, not ignored: a base with nothing placed in it
    /// answers with an empty list under its own id.
    #[tokio::test]
    async fn base_structures_are_empty_for_a_base_with_nothing_placed() {
        let mut test = TestContext::new(write_fixture_tree).await;
        test_with_one_structure(&mut test);

        run_handler_with_data!(
            test,
            handle_get_base_structures,
            json!({"base_id": FIXTURE_OTHER_BASE_ID})
        )
        .unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["data"]["base_id"], FIXTURE_OTHER_BASE_ID);
        assert_eq!(frame["data"]["structures"], json!([]));
    }

    /// A malformed request must be a visible error under `get_base_structures`
    /// (so `sendAndWait` still resolves), never an empty list that the map
    /// would render as "this base has no buildings".
    #[tokio::test]
    async fn base_structures_without_a_base_id_is_an_error_not_an_empty_list() {
        let mut test = TestContext::new(write_fixture_tree).await;
        test_with_one_structure(&mut test);

        run_handler_with_data!(test, handle_get_base_structures, json!({})).unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_base_structures");
        assert!(
            frame["data"]["error"].is_string(),
            "absent base_id must surface an error"
        );
        assert!(
            frame["data"].get("structures").is_none(),
            "absent base_id must not emit a structures list"
        );
        test.assert_no_more_frames();
    }

    /// Answers under `get_base_structures` even with no save loaded, so the
    /// frontend's request/response correlation still resolves.
    #[tokio::test]
    async fn base_structures_without_a_loaded_save_answer_with_an_error_field() {
        let mut test = TestContext::new(write_fixture_tree).await;

        run_handler_with_data!(
            test,
            handle_get_base_structures,
            json!({"base_id": FIXTURE_BASE_ID})
        )
        .unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_base_structures");
        assert_eq!(frame["data"]["base_id"], FIXTURE_BASE_ID);
        assert_eq!(frame["data"]["error"], "No save file loaded");
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
        let frame = run_handler!(test, handle_get_dungeons);
        assert_eq!(frame["type"], "get_dungeons");
        assert_eq!(frame["data"], json!({}));
    }

    #[tokio::test]
    async fn get_bosses_returns_the_raw_file() {
        let mut test = TestContext::new(write_fixture_tree).await;
        let frame = run_handler!(test, handle_get_bosses);
        assert_eq!(frame["type"], "get_bosses");
        assert!(
            frame["data"].is_object(),
            "bosses payload must be an object"
        );
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
    async fn map_layer_serves_one_layer_keyed_by_its_id() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(test, handle_get_map_layer, json!({"layers": ["notes"]})).unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_map_layer");
        assert_eq!(
            frame["data"]["layers"],
            json!({"notes": {"Day0": {"x": 8}}})
        );
        test.assert_no_more_frames();
    }

    async fn set_language(test: &TestContext, language: &str) {
        psp_db::settings::update_settings(
            &*test.app.driver,
            &psp_db::settings::SettingsUpdate {
                language: language.to_string(),
                clone_prefix: "©️".into(),
                new_pal_prefix: "🆕".into(),
                debug_mode: false,
                cheat_mode: false,
            },
        )
        .await
        .unwrap();
    }

    /// Driven by the l10n file's existence, not by a list of artifact ids.
    #[tokio::test]
    async fn map_layer_folds_localized_name_from_the_l10n_table() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(test, handle_get_map_layer, json!({"layers": ["towers"]})).unwrap();
        let frame = test.next_frame_json();
        assert_eq!(
            frame["data"]["layers"]["towers"]["Tower1"],
            json!({"class": "BP_PalBossTower_C", "x": 7,
                   "localized_name": "Rayne Syndicate Tower"})
        );
    }

    #[tokio::test]
    async fn map_layer_without_an_l10n_table_is_served_unchanged() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(test, handle_get_map_layer, json!({"layers": ["effigies"]}))
            .unwrap();
        let frame = test.next_frame_json();
        assert_eq!(
            frame["data"]["layers"]["effigies"],
            json!({"Eff1": {"x": 2}})
        );
    }

    /// `l10n/<lang>/relics.json` localizes `relic_data.json`, not the relic
    /// MARKER artifact of the same name: its keys are relic types, the markers'
    /// are instance ids. Folding a fallback onto every entry would stamp each
    /// marker with its own id as a display name.
    #[tokio::test]
    async fn map_layer_does_not_name_entries_the_l10n_table_does_not_cover() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(test, handle_get_map_layer, json!({"layers": ["relics"]})).unwrap();
        let frame = test.next_frame_json();
        assert_eq!(
            frame["data"]["layers"]["relics"]["Rel1"],
            json!({"x": 3, "relic_type": "jump_power"})
        );
    }

    /// Array-shaped artifacts carry no entry ids to key a merge on, so they
    /// pass through untouched even when a same-named l10n table exists.
    #[tokio::test]
    async fn map_layer_passes_array_shaped_artifacts_through_untouched() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(
            test,
            handle_get_map_layer,
            json!({"layers": ["camps", "eggs_spawners"]})
        )
        .unwrap();
        let frame = test.next_frame_json();
        assert_eq!(
            frame["data"]["layers"]["camps"],
            json!([{"class": "camp", "x": 11}])
        );
        assert_eq!(
            frame["data"]["layers"]["eggs_spawners"],
            json!([{"class": "spawner", "x": 9}])
        );
    }

    /// A language with no l10n tree degrades to the raw artifact rather than
    /// failing the request.
    #[tokio::test]
    async fn map_layer_in_a_language_without_l10n_serves_the_raw_artifact() {
        let mut test = TestContext::new(write_fixture_tree).await;
        set_language(&test, "de").await;
        run_handler_with_data!(test, handle_get_map_layer, json!({"layers": ["towers"]})).unwrap();
        let frame = test.next_frame_json();
        assert_eq!(
            frame["data"]["layers"]["towers"]["Tower1"],
            json!({"class": "BP_PalBossTower_C", "x": 7})
        );
    }

    /// One request, one response, every requested id echoed as a key — the
    /// client correlates only on the message type, so several layers must
    /// never be asked for as several concurrent requests.
    #[tokio::test]
    async fn map_layer_batches_every_requested_layer_into_one_response() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(
            test,
            handle_get_map_layer,
            json!({"layers": ["notes", "chests", "camps"]})
        )
        .unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_map_layer");
        assert_eq!(frame["data"]["layers"]["notes"], json!({"Day0": {"x": 8}}));
        assert_eq!(
            frame["data"]["layers"]["chests"],
            json!([{"class": "chest", "x": 10}])
        );
        assert_eq!(
            frame["data"]["layers"]["camps"],
            json!([{"class": "camp", "x": 11}])
        );
        test.assert_no_more_frames();
    }

    /// The three artifacts added after the first batch of layers. A layer left
    /// out of MAP_LAYERS is refused as an unknown id, so the allowlist is the
    /// only thing standing between a shipped artifact and an error frame.
    #[tokio::test]
    async fn map_layer_serves_the_later_artifacts() {
        let mut test = TestContext::new(|json_dir| {
            std::fs::write(
                json_dir.join("ancient_ruins.json"),
                r#"{"GUID1": {"class": "shrine", "x": 3}}"#,
            )
            .unwrap();
            std::fs::write(
                json_dir.join("kinship_peach.json"),
                r#"[{"name": "peach", "x": 5}]"#,
            )
            .unwrap();
            std::fs::write(
                json_dir.join("skill_fruits.json"),
                r#"[{"name": "fruit", "x": 7}]"#,
            )
            .unwrap();
        })
        .await;
        run_handler_with_data!(
            test,
            handle_get_map_layer,
            json!({"layers": ["ancient_ruins", "kinship_peach", "skill_fruits"]})
        )
        .unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_map_layer");
        assert!(
            frame["data"].get("error").is_none(),
            "these ids must be allowlisted, got: {}",
            frame["data"]
        );
        assert_eq!(
            frame["data"]["layers"]["ancient_ruins"],
            json!({"GUID1": {"class": "shrine", "x": 3}})
        );
        assert_eq!(
            frame["data"]["layers"]["kinship_peach"],
            json!([{"name": "peach", "x": 5}])
        );
        assert_eq!(
            frame["data"]["layers"]["skill_fruits"],
            json!([{"name": "fruit", "x": 7}])
        );
        test.assert_no_more_frames();
    }

    /// `get_map_layer` reads files off disk by name, so ids outside the
    /// allowlist are refused rather than resolved as a path.
    #[tokio::test]
    async fn map_layer_rejects_an_id_outside_the_allowlist() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(
            test,
            handle_get_map_layer,
            json!({"layers": ["towers", "../pals"]})
        )
        .unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_map_layer");
        assert!(
            frame["data"]["error"]
                .as_str()
                .is_some_and(|error| error.contains("../pals")),
            "the error must name the offending layer id, got: {}",
            frame["data"]
        );
        assert!(
            frame["data"].get("layers").is_none(),
            "a rejected request must not emit a partial layer map"
        );
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn map_layer_with_no_layers_requested_is_an_error() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(test, handle_get_map_layer, json!({"layers": []})).unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_map_layer");
        assert!(
            frame["data"]["error"].is_string(),
            "an empty request must surface an error"
        );
        assert!(frame["data"].get("layers").is_none());
        test.assert_no_more_frames();
    }

    /// A malformed payload still answers under `get_map_layer`, so a client
    /// waiting on that type resolves instead of hanging.
    #[tokio::test]
    async fn map_layer_with_a_malformed_payload_is_an_error() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(test, handle_get_map_layer, json!({})).unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_map_layer");
        assert!(frame["data"]["error"].is_string());
        test.assert_no_more_frames();
    }

    /// An allowlisted layer whose file is absent keeps `raw_file`'s empty-object
    /// answer: the key is still present, so the client resolves.
    #[tokio::test]
    async fn map_layer_answers_an_absent_artifact_with_an_empty_object() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(test, handle_get_map_layer, json!({"layers": ["dungeons"]}))
            .unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["data"]["layers"]["dungeons"], json!({}));
    }

    /// `fast_travel_points` reaches the same shape through either message, so a
    /// layer moved onto `get_map_layer` does not lose its names.
    #[tokio::test]
    async fn map_layer_and_get_fast_travel_points_agree_on_shape() {
        let mut test = TestContext::new(write_fixture_tree).await;
        run_handler_with_data!(
            test,
            handle_get_map_layer,
            json!({"layers": ["fast_travel_points"]})
        )
        .unwrap();
        let batched = test.next_frame_json();
        assert_eq!(
            batched["data"]["layers"]["fast_travel_points"]["FT1"],
            json!({"x": 1, "localized_name": "Beach"})
        );

        let bespoke = run_handler!(test, handle_get_fast_travel_points);
        assert_eq!(
            batched["data"]["layers"]["fast_travel_points"],
            bespoke["data"]
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
