use serde_json::Value;

use crate::bridge::service::BridgeError;
use crate::dispatcher::HandlerCtx;
use crate::emitter::Emitter;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::services::ServerServices;

fn parse_payload<T: serde::de::DeserializeOwned>(
    data: Value,
    request: MessageType,
    ctx: &mut HandlerCtx<'_>,
) -> Option<T> {
    match serde_json::from_value(data) {
        Ok(payload) => Some(payload),
        Err(error) => {
            ctx.emitter.emit(
                request,
                &serde_json::json!({ "error": error.to_string(), "code": "validation_failed" }),
            );
            None
        }
    }
}

pub async fn handle_game_status(
    services: &ServerServices,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match services
        .bridge
        .request("get_status", serde_json::json!({}))
        .await
    {
        Ok(payload) => ctx.emitter.emit(MessageType::GameStatus, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameStatus, &error),
    }
    Ok(())
}

pub async fn handle_game_players(
    services: &ServerServices,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match services
        .bridge
        .request("get_players", serde_json::json!({}))
        .await
    {
        Ok(payload) => ctx.emitter.emit(MessageType::GamePlayers, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GamePlayers, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GamePalsData {
    pub player_uid: String,
    #[serde(default)]
    pub page: i64,
}

pub async fn handle_game_pals(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GamePalsData>(data, MessageType::GamePals, ctx) else {
        return Ok(());
    };
    let request = serde_json::json!({ "playerUid": data.player_uid, "page": data.page });
    match services.bridge.request("get_pals", request).await {
        Ok(payload) => ctx.emitter.emit(MessageType::GamePals, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GamePals, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GamePalDetailData {
    pub player_uid: String,
    #[serde(default)]
    pub slot_index: Option<i64>,
    #[serde(default)]
    pub instance_id: Option<String>,
}

fn refuse_without_pal_address(
    slot_index: Option<i64>,
    instance_id: Option<&String>,
    request: MessageType,
    ctx: &mut HandlerCtx<'_>,
) -> bool {
    if slot_index.is_some() == instance_id.is_some() {
        ctx.emitter.emit(
            request,
            &serde_json::json!({
                "error": "exactly one of slot_index or instance_id is required",
                "code": "validation_failed",
            }),
        );
        return true;
    }
    false
}

pub async fn handle_game_pal_detail(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GamePalDetailData>(data, MessageType::GamePalDetail, ctx)
    else {
        return Ok(());
    };
    if refuse_without_pal_address(
        data.slot_index,
        data.instance_id.as_ref(),
        MessageType::GamePalDetail,
        ctx,
    ) {
        return Ok(());
    }
    let request = serde_json::json!({
        "playerUid": data.player_uid,
        "slotIndex": data.slot_index,
        "instanceId": data.instance_id,
    });
    match services.bridge.request("get_pal_detail", request).await {
        Ok(payload) => ctx.emitter.emit(MessageType::GamePalDetail, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GamePalDetail, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameInventoryData {
    pub player_uid: String,
}

pub async fn handle_game_inventory(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameInventoryData>(data, MessageType::GameInventory, ctx)
    else {
        return Ok(());
    };
    let request = serde_json::json!({ "playerUid": data.player_uid });
    match services.bridge.request("get_inventory", request).await {
        Ok(payload) => ctx.emitter.emit(MessageType::GameInventory, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameInventory, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameGuildData {
    #[serde(default)]
    pub player_uid: Option<String>,
    #[serde(default)]
    pub guild_id: Option<String>,
}

pub async fn handle_game_guilds(
    services: &ServerServices,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match services
        .bridge
        .request("get_guilds", serde_json::json!({}))
        .await
    {
        Ok(payload) => ctx.emitter.emit(MessageType::GameGuilds, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameGuilds, &error),
    }
    Ok(())
}

pub async fn handle_game_guild(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameGuildData>(data, MessageType::GameGuild, ctx) else {
        return Ok(());
    };
    let request = match (&data.guild_id, &data.player_uid) {
        (Some(guild_id), _) => serde_json::json!({ "guildId": guild_id }),
        (None, Some(player_uid)) => serde_json::json!({ "playerUid": player_uid }),
        (None, None) => serde_json::json!({}),
    };
    match services.bridge.request("get_guild", request).await {
        Ok(payload) => ctx.emitter.emit(MessageType::GameGuild, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameGuild, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameBasePalsData {
    pub base_id: String,
}

pub async fn handle_game_base_pals(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameBasePalsData>(data, MessageType::GameBasePals, ctx) else {
        return Ok(());
    };
    let request = serde_json::json!({ "baseId": data.base_id });
    match services.bridge.request("get_base_pals", request).await {
        Ok(payload) => ctx.emitter.emit(MessageType::GameBasePals, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameBasePals, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameGuildContainersData {
    pub guild_id: String,
}

pub async fn handle_game_guild_containers(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) =
        parse_payload::<GameGuildContainersData>(data, MessageType::GameGuildContainers, ctx)
    else {
        return Ok(());
    };
    let request = serde_json::json!({ "guildId": data.guild_id });
    match services
        .bridge
        .request("get_guild_containers", request)
        .await
    {
        Ok(payload) => ctx.emitter.emit(MessageType::GameGuildContainers, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameGuildContainers, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameEditGuildData {
    pub guild_id: String,
    pub base_camp_level: i64,
    #[serde(default)]
    pub command_id: Option<String>,
}

pub async fn handle_game_edit_guild(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameEditGuildData>(data, MessageType::GameEditGuild, ctx)
    else {
        return Ok(());
    };
    let command_id = data
        .command_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let args = serde_json::json!({
        "guildId": data.guild_id,
        "baseCampLevel": data.base_camp_level,
    });
    match services
        .bridge
        .command("guild.edit", &command_id, args)
        .await
    {
        Ok(payload) => ctx.emitter.emit(MessageType::GameEditGuild, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameEditGuild, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameSetGuildRoleData {
    pub guild_id: String,
    pub member_uid: String,
    pub role: String,
    #[serde(default)]
    pub command_id: Option<String>,
}

pub async fn handle_game_set_guild_role(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) =
        parse_payload::<GameSetGuildRoleData>(data, MessageType::GameSetGuildRole, ctx)
    else {
        return Ok(());
    };
    let command_id = data
        .command_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let args = serde_json::json!({
        "guildId": data.guild_id,
        "memberUid": data.member_uid,
        "role": data.role,
    });
    match services
        .bridge
        .command("guild.setRole", &command_id, args)
        .await
    {
        Ok(payload) => ctx.emitter.emit(MessageType::GameSetGuildRole, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameSetGuildRole, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameHealPalsData {
    pub player_uid: String,
    pub targets: Vec<HealTarget>,
    #[serde(default)]
    pub command_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct HealTarget {
    pub slot_index: i64,
}

const MAX_HEAL_TARGETS: usize = 30;

pub async fn handle_game_heal_pals(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameHealPalsData>(data, MessageType::GameHealPals, ctx) else {
        return Ok(());
    };
    if data.targets.is_empty() || data.targets.len() > MAX_HEAL_TARGETS {
        ctx.emitter.emit(
            MessageType::GameHealPals,
            &serde_json::json!({
                "error": format!(
                    "targets must contain 1 to {MAX_HEAL_TARGETS} entries, got {}",
                    data.targets.len()
                ),
                "code": "validation_failed",
            }),
        );
        return Ok(());
    }

    let base = data
        .command_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut results = Vec::with_capacity(data.targets.len());
    for (index, target) in data.targets.iter().enumerate() {
        let command_id = format!("{base}:{}", target.slot_index);
        let args = serde_json::json!({
            "playerUid": data.player_uid,
            "slotIndex": target.slot_index,
        });
        match services.bridge.command("pal.heal", &command_id, args).await {
            Ok(payload) => results.push(serde_json::json!({
                "slot_index": target.slot_index,
                "ok": true,
                "result": payload,
                "error": null,
            })),
            Err(error) => {
                if index == 0 && matches!(error, BridgeError::Offline | BridgeError::Timeout) {
                    refuse_bridge_error(ctx.emitter, MessageType::GameHealPals, &error);
                    return Ok(());
                }
                let (message, code) = bridge_error_parts(&error);
                results.push(serde_json::json!({
                    "slot_index": target.slot_index,
                    "ok": false,
                    "result": null,
                    "error": { "code": code, "message": message },
                }));
            }
        }
    }
    ctx.emitter.emit(
        MessageType::GameHealPals,
        &serde_json::json!({ "results": results }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameSetItemSlotData {
    pub player_uid: String,
    pub container_id: String,
    pub slot_index: i64,
    #[serde(default)]
    pub static_item_id: Option<String>,
    #[serde(default)]
    pub count: Option<i64>,
    #[serde(default)]
    pub command_id: Option<String>,
}

pub async fn handle_game_set_item_slot(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameSetItemSlotData>(data, MessageType::GameSetItemSlot, ctx)
    else {
        return Ok(());
    };
    let command_id = data
        .command_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let args = serde_json::json!({
        "playerUid": data.player_uid,
        "containerId": data.container_id,
        "slotIndex": data.slot_index,
        "staticItemId": data.static_item_id,
        "count": data.count,
    });
    match services
        .bridge
        .command("item.setSlot", &command_id, args)
        .await
    {
        Ok(payload) => ctx.emitter.emit(MessageType::GameSetItemSlot, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameSetItemSlot, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameRemovePalData {
    pub player_uid: String,
    pub slot_index: i64,
    #[serde(default)]
    pub command_id: Option<String>,
}

pub async fn handle_game_remove_pal(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameRemovePalData>(data, MessageType::GameRemovePal, ctx)
    else {
        return Ok(());
    };
    let command_id = data
        .command_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let args = serde_json::json!({
        "playerUid": data.player_uid,
        "slotIndex": data.slot_index,
    });
    match services
        .bridge
        .command("pal.remove", &command_id, args)
        .await
    {
        Ok(payload) => ctx.emitter.emit(MessageType::GameRemovePal, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameRemovePal, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameMovePalData {
    pub player_uid: String,
    pub from_slot_index: i64,
    pub to_slot_index: i64,
    #[serde(default)]
    pub command_id: Option<String>,
}

pub async fn handle_game_move_pal(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameMovePalData>(data, MessageType::GameMovePal, ctx) else {
        return Ok(());
    };
    let command_id = data
        .command_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let args = serde_json::json!({
        "playerUid": data.player_uid,
        "fromSlotIndex": data.from_slot_index,
        "toSlotIndex": data.to_slot_index,
    });
    match services.bridge.command("pal.move", &command_id, args).await {
        Ok(payload) => ctx.emitter.emit(MessageType::GameMovePal, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameMovePal, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameAddPalData {
    pub player_uid: String,
    #[serde(default)]
    pub slot_index: Option<i64>,
    pub character_id: String,
    #[serde(default)]
    pub level: Option<i64>,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub base_id: Option<String>,
    #[serde(default)]
    pub party: Option<bool>,
    #[serde(default)]
    pub command_id: Option<String>,
}

fn refuse_without_add_target(
    slot_index: Option<i64>,
    base_id: Option<&String>,
    party: bool,
    ctx: &mut HandlerCtx<'_>,
) -> bool {
    let error = if party && base_id.is_some() {
        "a pal goes to one place: base_id or party, not both"
    } else if !party && slot_index.is_none() {
        "slot_index is required unless the pal goes to the party"
    } else {
        return false;
    };
    ctx.emitter.emit(
        MessageType::GameAddPal,
        &serde_json::json!({ "error": error, "code": "validation_failed" }),
    );
    true
}

pub async fn handle_game_add_pal(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameAddPalData>(data, MessageType::GameAddPal, ctx) else {
        return Ok(());
    };
    let party = data.party.unwrap_or(false);
    if refuse_without_add_target(data.slot_index, data.base_id.as_ref(), party, ctx) {
        return Ok(());
    }
    let command_id = data
        .command_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let args = serde_json::json!({
        "playerUid": data.player_uid,
        "slotIndex": data.slot_index,
        "characterId": data.character_id,
        "level": data.level,
        "gender": data.gender,
        "baseId": data.base_id,
        "party": data.party,
    });
    match services.bridge.command("pal.add", &command_id, args).await {
        Ok(payload) => ctx.emitter.emit(MessageType::GameAddPal, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameAddPal, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameEditPalData {
    pub player_uid: String,
    #[serde(default)]
    pub slot_index: Option<i64>,
    #[serde(default)]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub level: Option<i64>,
    #[serde(default)]
    pub rank: Option<i64>,
    #[serde(default)]
    pub exp: Option<i64>,
    #[serde(default)]
    pub talent_hp: Option<i64>,
    #[serde(default)]
    pub talent_melee: Option<i64>,
    #[serde(default)]
    pub talent_shot: Option<i64>,
    #[serde(default)]
    pub talent_defense: Option<i64>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub rank_hp: Option<i64>,
    #[serde(default)]
    pub rank_attack: Option<i64>,
    #[serde(default)]
    pub rank_defense: Option<i64>,
    #[serde(default)]
    pub rank_craft_speed: Option<i64>,
    #[serde(default)]
    pub active_skills: Option<Vec<String>>,
    #[serde(default)]
    pub passive_skills: Option<Vec<String>>,
    #[serde(default)]
    pub work_suitability: Option<Value>,
    #[serde(default)]
    pub friendship_point: Option<i64>,
    #[serde(default)]
    pub sanity: Option<f64>,
    #[serde(default)]
    pub stomach: Option<f64>,
    #[serde(default)]
    pub hp: Option<i64>,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub is_awakened: Option<bool>,
    #[serde(default)]
    pub is_lucky: Option<bool>,
    #[serde(default)]
    pub command_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GameEditPlayerData {
    pub player_uid: String,
    #[serde(default)]
    pub command_id: Option<String>,
    #[serde(default)]
    pub level: Option<i64>,
    #[serde(default)]
    pub exp: Option<i64>,
}

pub async fn handle_game_edit_pal(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameEditPalData>(data, MessageType::GameEditPal, ctx) else {
        return Ok(());
    };
    if refuse_without_pal_address(
        data.slot_index,
        data.instance_id.as_ref(),
        MessageType::GameEditPal,
        ctx,
    ) {
        return Ok(());
    }
    let command_id = data
        .command_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let args = serde_json::json!({
        "playerUid": data.player_uid,
        "slotIndex": data.slot_index,
        "instanceId": data.instance_id,
        "level": data.level,
        "rank": data.rank,
        "exp": data.exp,
        "talentHp": data.talent_hp,
        "talentMelee": data.talent_melee,
        "talentShot": data.talent_shot,
        "talentDefense": data.talent_defense,
        "nickname": data.nickname,
        "rankHp": data.rank_hp,
        "rankAttack": data.rank_attack,
        "rankDefense": data.rank_defense,
        "rankCraftSpeed": data.rank_craft_speed,
        "activeSkills": data.active_skills,
        "passiveSkills": data.passive_skills,
        "workSuitability": data.work_suitability,
        "friendshipPoint": data.friendship_point,
        "sanity": data.sanity,
        "stomach": data.stomach,
        "hp": data.hp,
        "gender": data.gender,
        "isAwakened": data.is_awakened,
        "isLucky": data.is_lucky,
    });
    match services.bridge.command("pal.edit", &command_id, args).await {
        Ok(payload) => ctx.emitter.emit(MessageType::GameEditPal, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameEditPal, &error),
    }
    Ok(())
}

pub async fn handle_game_edit_player(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(data) = parse_payload::<GameEditPlayerData>(data, MessageType::GameEditPlayer, ctx)
    else {
        return Ok(());
    };
    let command_id = data
        .command_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let args = serde_json::json!({
        "playerUid": data.player_uid,
        "level": data.level,
        "exp": data.exp,
    });
    match services
        .bridge
        .command("player.edit", &command_id, args)
        .await
    {
        Ok(payload) => ctx.emitter.emit(MessageType::GameEditPlayer, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameEditPlayer, &error),
    }
    Ok(())
}

pub async fn handle_game_capabilities(
    services: &ServerServices,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match services.bridge.get_capabilities().await {
        Ok(payload) => ctx.emitter.emit(MessageType::GameCapabilities, &payload),
        Err(error) => refuse_bridge_error(ctx.emitter, MessageType::GameCapabilities, &error),
    }
    Ok(())
}

fn bridge_error_parts(error: &BridgeError) -> (String, String) {
    match error {
        BridgeError::Offline | BridgeError::Transport => {
            (error.to_string(), "bridge_offline".to_string())
        }
        BridgeError::Timeout => (error.to_string(), "timeout".to_string()),
        BridgeError::Mod { code, message } => (message.clone(), code.clone()),
    }
}

fn refuse_bridge_error(emitter: &Emitter, request: MessageType, error: &BridgeError) {
    let (message, code) = bridge_error_parts(error);
    emitter.emit(
        request,
        &serde_json::json!({ "error": message, "code": code }),
    );
}
