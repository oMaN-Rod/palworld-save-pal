//! PalWorldSettings.ini generation for native servers. The file holds a single
//! `[/Script/Pal.PalGameWorldSettings]` section whose `OptionSettings=(...)` line
//! is one comma-separated `Key=Value` list; strings are quoted, bools are
//! `True`/`False`, and unknown keys are ignored by the server.
use std::path::{Path, PathBuf};

use psp_db::servers::ServerRecord;

pub fn saves_path(install_path: &str) -> String {
    Path::new(install_path)
        .join("Pal")
        .join("Saved")
        .to_string_lossy()
        .to_string()
}

pub fn mods_path(install_path: &str) -> String {
    Path::new(install_path)
        .join("Pal")
        .join("Binaries")
        .join("Win64")
        .join("Mods")
        .to_string_lossy()
        .to_string()
}

pub fn logicmods_path(install_path: &str) -> String {
    Path::new(install_path)
        .join("Pal")
        .join("Content")
        .join("Paks")
        .join("LogicMods")
        .to_string_lossy()
        .to_string()
}

pub fn nativemods_path(install_path: &str) -> String {
    Path::new(install_path)
        .join("Pal")
        .join("Binaries")
        .join("Win64")
        .join("NativeMods")
        .to_string_lossy()
        .to_string()
}

/// ENV var key -> OptionSettings key. Unmapped keys are skipped when writing the
/// ini. Several ini keys are misspelled by the game itself (Decreace, Regene) —
/// the spellings here are what the server actually reads.
const ENV_TO_INI: &[(&str, &str)] = &[
    ("SERVER_NAME", "ServerName"),
    ("SERVER_DESCRIPTION", "ServerDescription"),
    ("SERVER_PASSWORD", "ServerPassword"),
    ("ADMIN_PASSWORD", "AdminPassword"),
    ("PLAYERS", "ServerPlayerMaxNum"),
    ("PORT", "PublicPort"),
    ("EXP_RATE", "ExpRate"),
    ("PAL_CAPTURE_RATE", "PalCaptureRate"),
    ("PAL_SPAWN_NUM_RATE", "PalSpawnNumRate"),
    ("PAL_DAMAGE_RATE_ATTACK", "PalDamageRateAttack"),
    ("PAL_DAMAGE_RATE_DEFENSE", "PalDamageRateDefense"),
    ("PLAYER_DAMAGE_RATE_ATTACK", "PlayerDamageRateAttack"),
    ("PLAYER_DAMAGE_RATE_DEFENSE", "PlayerDamageRateDefense"),
    ("PAL_STOMACH_DECREASE_RATE", "PalStomachDecreaceRate"),
    ("PAL_STAMINA_DECREASE_RATE", "PalStaminaDecreaceRate"),
    ("PAL_AUTO_HP_REGEN_RATE", "PalAutoHPRegeneRate"),
    (
        "PAL_AUTO_HP_REGEN_RATE_IN_SLEEP",
        "PalAutoHpRegeneRateInSleep",
    ),
    ("PLAYER_STOMACH_DECREASE_RATE", "PlayerStomachDecreaceRate"),
    ("PLAYER_STAMINA_DECREASE_RATE", "PlayerStaminaDecreaceRate"),
    ("PLAYER_AUTO_HP_REGEN_RATE", "PlayerAutoHPRegeneRate"),
    (
        "PLAYER_AUTO_HP_REGEN_RATE_IN_SLEEP",
        "PlayerAutoHpRegeneRateInSleep",
    ),
    ("COLLECTION_DROP_RATE", "CollectionDropRate"),
    ("COLLECTION_OBJECT_HP_RATE", "CollectionObjectHpRate"),
    (
        "COLLECTION_OBJECT_RESPAWN_SPEED_RATE",
        "CollectionObjectRespawnSpeedRate",
    ),
    ("ENEMY_DROP_ITEM_RATE", "EnemyDropItemRate"),
    ("WORK_SPEED_RATE", "WorkSpeedRate"),
    ("ITEM_WEIGHT_RATE", "ItemWeightRate"),
    (
        "EQUIPMENT_DURABILITY_DAMAGE_RATE",
        "EquipmentDurabilityDamageRate",
    ),
    ("ITEM_CORRUPTION_MULTIPLIER", "ItemCorruptionMultiplier"),
    ("DIFFICULTY", "Difficulty"),
    ("DAYTIME_SPEEDRATE", "DayTimeSpeedRate"),
    ("NIGHTTIME_SPEEDRATE", "NightTimeSpeedRate"),
    ("PAL_EGG_DEFAULT_HATCHING_TIME", "PalEggDefaultHatchingTime"),
    ("AUTO_SAVE_SPAN", "AutoSaveSpan"),
    ("DROP_ITEM_ALIVE_MAX_HOURS", "DropItemAliveMaxHours"),
    ("SUPPLY_DROP_SPAN", "SupplyDropSpan"),
    ("PUBLIC_IP", "PublicIP"),
    ("PUBLIC_PORT", "PublicPort"),
    ("REGION", "Region"),
    ("USEAUTH", "bUseAuth"),
    ("SHOW_PLAYER_LIST", "bShowPlayerList"),
    ("SHOW_JOIN_LEFT_MESSAGE", "bIsShowJoinLeftMessage"),
    ("ALLOW_CLIENT_MOD", "bAllowClientMod"),
    ("CHAT_POST_LIMIT_PER_MINUTE", "ChatPostLimitPerMinute"),
    ("BAN_LIST_URL", "BanListURL"),
    ("CROSSPLAY_PLATFORMS", "CrossplayPlatforms"),
    ("ALLOW_ENHANCE_STAT_HEALTH", "bAllowEnhanceStat_Health"),
    ("ALLOW_ENHANCE_STAT_ATTACK", "bAllowEnhanceStat_Attack"),
    ("ALLOW_ENHANCE_STAT_STAMINA", "bAllowEnhanceStat_Stamina"),
    ("ALLOW_ENHANCE_STAT_WEIGHT", "bAllowEnhanceStat_Weight"),
    (
        "ALLOW_ENHANCE_STAT_WORK_SPEED",
        "bAllowEnhanceStat_WorkSpeed",
    ),
    ("IS_PVP", "bIsPvP"),
    (
        "ENABLE_PLAYER_TO_PLAYER_DAMAGE",
        "bEnablePlayerToPlayerDamage",
    ),
    ("ENABLE_FRIENDLY_FIRE", "bEnableFriendlyFire"),
    (
        "ENABLE_DEFENSE_OTHER_GUILD_PLAYER",
        "bEnableDefenseOtherGuildPlayer",
    ),
    ("HARDCORE", "bHardcore"),
    (
        "CHARACTER_RECREATE_IN_HARDCORE",
        "bCharacterRecreateInHardcore",
    ),
    ("PAL_LOST", "bPalLost"),
    ("DEATH_PENALTY", "DeathPenalty"),
    (
        "CAN_PICKUP_OTHER_GUILD_DEATH_PENALTY_DROP",
        "bCanPickupOtherGuildDeathPenaltyDrop",
    ),
    ("ENABLE_AIM_ASSIST_PAD", "bEnableAimAssistPad"),
    ("ENABLE_AIM_ASSIST_KEYBOARD", "bEnableAimAssistKeyboard"),
    ("ENABLE_INVADER_ENEMY", "bEnableInvaderEnemy"),
    ("ENABLE_PREDATOR_BOSS_PAL", "EnablePredatorBossPal"),
    ("ENABLE_NON_LOGIN_PENALTY", "bEnableNonLoginPenalty"),
    ("ENABLE_FAST_TRAVEL", "bEnableFastTravel"),
    (
        "ENABLE_FAST_TRAVEL_ONLY_BASE_CAMP",
        "bEnableFastTravelOnlyBaseCamp",
    ),
    ("EXIST_PLAYER_AFTER_LOGOUT", "bExistPlayerAfterLogout"),
    (
        "IS_START_LOCATION_SELECT_BY_MAP",
        "bIsStartLocationSelectByMap",
    ),
    ("BLOCK_RESPAWN_TIME", "BlockRespawnTime"),
    (
        "RESPAWN_PENALTY_DURATION_THRESHOLD",
        "RespawnPenaltyDurationThreshold",
    ),
    ("RESPAWN_PENALTY_TIME_SCALE", "RespawnPenaltyTimeScale"),
    (
        "ADDITIONAL_DROP_ITEM_WHEN_PLAYER_KILLING_IN_PVP",
        "bAdditionalDropItemWhenPlayerKillingInPvPMode",
    ),
    (
        "ADDITIONAL_DROP_ITEM_PVP_ITEM",
        "AdditionalDropItemWhenPlayerKillingInPvPMode",
    ),
    (
        "ADDITIONAL_DROP_ITEM_PVP_NUM",
        "AdditionalDropItemNumWhenPlayerKillingInPvPMode",
    ),
    (
        "DISPLAY_PVP_ITEM_NUM_ON_WORLDMAP_BASECAMP",
        "bDisplayPvPItemNumOnWorldMap_BaseCamp",
    ),
    (
        "DISPLAY_PVP_ITEM_NUM_ON_WORLDMAP_PLAYER",
        "bDisplayPvPItemNumOnWorldMap_Player",
    ),
    ("GUILD_PLAYER_MAX_NUM", "GuildPlayerMaxNum"),
    (
        "GUILD_REJOIN_COOLDOWN_MINUTES",
        "GuildRejoinCooldownMinutes",
    ),
    ("BASE_CAMP_MAX_NUM", "BaseCampMaxNum"),
    ("BASE_CAMP_MAX_NUM_IN_GUILD", "BaseCampMaxNumInGuild"),
    ("BASE_CAMP_WORKER_MAX_NUM", "BaseCampWorkerMaxNum"),
    ("BUILD_OBJECT_HP_RATE", "BuildObjectHpRate"),
    ("BUILD_OBJECT_DAMAGE_RATE", "BuildObjectDamageRate"),
    (
        "BUILD_OBJECT_DETERIORATION_DAMAGE_RATE",
        "BuildObjectDeteriorationDamageRate",
    ),
    ("BUILD_AREA_LIMIT", "bBuildAreaLimit"),
    ("MAX_BUILDING_LIMIT_NUM", "MaxBuildingLimitNum"),
    (
        "AUTO_RESET_GUILD_NO_ONLINE_PLAYERS",
        "bAutoResetGuildNoOnlinePlayers",
    ),
    (
        "AUTO_RESET_GUILD_TIME_NO_ONLINE_PLAYERS",
        "AutoResetGuildTimeNoOnlinePlayers",
    ),
    (
        "INVISIBLE_OTHER_GUILD_BASE_CAMP_AREA_FX",
        "bInvisibleOtherGuildBaseCampAreaFX",
    ),
    ("DROP_ITEM_MAX_NUM", "DropItemMaxNum"),
    ("DROP_ITEM_MAX_NUM_UNKO", "DropItemMaxNum_UNKO"),
    ("ACTIVE_UNKO", "bActiveUNKO"),
    ("COOP_PLAYER_MAX_NUM", "CoopPlayerMaxNum"),
    ("ALLOW_GLOBAL_PALBOX_EXPORT", "bAllowGlobalPalboxExport"),
    ("ALLOW_GLOBAL_PALBOX_IMPORT", "bAllowGlobalPalboxImport"),
    ("IS_MULTIPLAY", "bIsMultiplay"),
    ("REST_API_ENABLED", "RESTAPIEnabled"),
    ("REST_API_PORT", "RESTAPIPort"),
    ("RCON_ENABLED", "RCONEnabled"),
    ("RCON_PORT", "RCONPort"),
    ("LOG_FORMAT_TYPE", "LogFormatType"),
    ("USE_BACKUP_SAVE_DATA", "bIsUseBackupSaveData"),
    (
        "SERVER_REPLICATE_PAWN_CULL_DISTANCE",
        "ServerReplicatePawnCullDistance",
    ),
    (
        "ITEM_CONTAINER_FORCE_MARK_DIRTY_INTERVAL",
        "ItemContainerForceMarkDirtyInterval",
    ),
    ("RANDOMIZER_TYPE", "RandomizerType"),
    ("RANDOMIZER_SEED", "RandomizerSeed"),
    (
        "IS_RANDOMIZER_PAL_LEVEL_RANDOM",
        "bIsRandomizerPalLevelRandom",
    ),
];

/// Consumed by the Docker image's entrypoint, not by the game — never written to
/// PalWorldSettings.ini.
const DOCKER_ONLY_KEYS: &[&str] = &[
    "MULTITHREADING",
    "COMMUNITY",
    "UPDATE_ON_BOOT",
    "ENABLE_PLAYER_LOGGING",
    "PLAYER_LOGGING_POLL_PERIOD",
    "LOG_FILTER_ENABLED",
    "BACKUP_ENABLED",
    "BACKUP_CRON_EXPRESSION",
    "DELETE_OLD_BACKUPS",
    "OLD_BACKUP_DAYS",
    "AUTO_UPDATE_ENABLED",
    "AUTO_UPDATE_CRON_EXPRESSION",
    "AUTO_UPDATE_WARN_MINUTES",
    "AUTO_REBOOT_ENABLED",
    "AUTO_REBOOT_CRON_EXPRESSION",
    "AUTO_REBOOT_WARN_MINUTES",
    "AUTO_REBOOT_EVEN_IF_PLAYERS_ONLINE",
    "USE_DEPOT_DOWNLOADER",
    "INSTALL_BETA_INSIDER",
    "DISCORD_WEBHOOK_URL",
    "DISCORD_SUPPRESS_NOTIFICATIONS",
    "DISCORD_CONNECT_TIMEOUT",
    "DISCORD_MAX_TIMEOUT",
    "ENABLE_UE4SS",
    "UE4SS_VERSION",
    "UE4SS_FORCE_UPDATE",
    "LAN_SERVER_MAX_TICK_RATE",
    "NET_SERVER_MAX_TICK_RATE",
    "SMOOTH_FRAME_RATE",
    "SMOOTH_FRAME_RATE_UPPER_LIMIT",
    "SMOOTH_FRAME_RATE_LOWER_LIMIT",
];

const BOOL_INI_KEYS: &[&str] = &[
    "bUseAuth",
    "bShowPlayerList",
    "bIsShowJoinLeftMessage",
    "bAllowClientMod",
    "bAllowEnhanceStat_Health",
    "bAllowEnhanceStat_Attack",
    "bAllowEnhanceStat_Stamina",
    "bAllowEnhanceStat_Weight",
    "bAllowEnhanceStat_WorkSpeed",
    "bIsPvP",
    "bEnablePlayerToPlayerDamage",
    "bEnableFriendlyFire",
    "bEnableDefenseOtherGuildPlayer",
    "bHardcore",
    "bCharacterRecreateInHardcore",
    "bPalLost",
    "bCanPickupOtherGuildDeathPenaltyDrop",
    "bEnableAimAssistPad",
    "bEnableAimAssistKeyboard",
    "bEnableInvaderEnemy",
    "EnablePredatorBossPal",
    "bEnableNonLoginPenalty",
    "bEnableFastTravel",
    "bEnableFastTravelOnlyBaseCamp",
    "bExistPlayerAfterLogout",
    "bIsStartLocationSelectByMap",
    "bBuildAreaLimit",
    "bAutoResetGuildNoOnlinePlayers",
    "bInvisibleOtherGuildBaseCampAreaFX",
    "bActiveUNKO",
    "bAllowGlobalPalboxExport",
    "bAllowGlobalPalboxImport",
    "bIsMultiplay",
    "RESTAPIEnabled",
    "RCONEnabled",
    "bIsUseBackupSaveData",
    "bIsRandomizerPalLevelRandom",
    "bAdditionalDropItemWhenPlayerKillingInPvPMode",
    "bDisplayPvPItemNumOnWorldMap_BaseCamp",
    "bDisplayPvPItemNumOnWorldMap_Player",
];

const STRING_INI_KEYS: &[&str] = &[
    "ServerName",
    "ServerDescription",
    "ServerPassword",
    "AdminPassword",
    "PublicIP",
    "Region",
    "BanListURL",
    "DeathPenalty",
    "Difficulty",
    "LogFormatType",
    "RandomizerSeed",
    "AdditionalDropItemWhenPlayerKillingInPvPMode",
];

pub fn env_to_ini_key(env_key: &str) -> Option<&'static str> {
    ENV_TO_INI
        .iter()
        .find(|(env, _)| *env == env_key)
        .map(|(_, ini)| *ini)
}

pub fn is_docker_only_key(env_key: &str) -> bool {
    DOCKER_ONLY_KEYS.contains(&env_key)
}

pub fn format_ini_value(ini_key: &str, value: &str) -> String {
    if STRING_INI_KEYS.contains(&ini_key) {
        if value.starts_with('"') && value.ends_with('"') {
            return value.to_string();
        }
        return format!("\"{value}\"");
    }
    if ini_key == "CrossplayPlatforms" || ini_key == "DenyTechnologyList" {
        return value.to_string();
    }
    if BOOL_INI_KEYS.contains(&ini_key) {
        match value.to_ascii_lowercase().as_str() {
            "true" | "1" => return "True".to_string(),
            "false" | "0" => return "False".to_string(),
            _ => {}
        }
    }
    value.to_string()
}

/// Splits the `OptionSettings` list on commas, ignoring those inside quoted
/// strings and parenthesized tuples like `CrossplayPlatforms=(Steam,Xbox)`.
pub fn split_option_settings(options: &str) -> Vec<String> {
    let mut pairs = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut in_quote = false;
    for character in options.chars() {
        match character {
            '"' if depth == 0 => {
                in_quote = !in_quote;
                current.push(character);
            }
            '(' if !in_quote => {
                depth += 1;
                current.push(character);
            }
            ')' if !in_quote => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if !in_quote && depth == 0 => {
                pairs.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(character),
        }
    }
    if !current.is_empty() {
        pairs.push(current.trim().to_string());
    }
    pairs
}

/// Parses the `OptionSettings=(...)` list from an ini file. Returns `None` when
/// the file is missing or has no OptionSettings line.
pub fn parse_option_settings_ini(path: &Path) -> Option<Vec<(String, String)>> {
    let contents = std::fs::read_to_string(path).ok()?;
    let start_index = contents.find("OptionSettings=(")?;
    let after_open = start_index + "OptionSettings=(".len();
    let close_offset = contents[after_open..].find(')')?;
    let options = &contents[after_open..after_open + close_offset];
    let mut pairs = Vec::new();
    for pair in split_option_settings(options) {
        if let Some((key, value)) = pair.split_once('=') {
            if !key.trim().is_empty() {
                pairs.push((key.trim().to_string(), value.trim().to_string()));
            }
        }
    }
    Some(pairs)
}

/// Reads the server's own DefaultPalWorldSettings.ini (shipped at the install
/// root), falling back to [`hardcoded_defaults`] when it is missing or malformed.
pub fn parse_default_settings(default_ini_path: &Path) -> Vec<(String, String)> {
    parse_option_settings_ini(default_ini_path).unwrap_or_else(hardcoded_defaults)
}

pub fn ini_to_env_key(ini_key: &str) -> Option<&'static str> {
    ENV_TO_INI
        .iter()
        .find(|(_, ini)| *ini == ini_key)
        .map(|(env, _)| *env)
}

#[derive(Debug, Default, Clone)]
pub struct ImportedServerConfig {
    pub server_name: String,
    pub server_description: String,
    pub server_password: String,
    pub admin_password: String,
    pub max_players: i64,
    pub game_port: i64,
    pub rest_api_port: i64,
    pub env_vars: serde_json::Map<String, serde_json::Value>,
}

fn ini_value_to_env(ini_key: &str, value: &str) -> String {
    let trimmed = value.trim();
    if STRING_INI_KEYS.contains(&ini_key) {
        return trimmed.trim_matches('"').to_string();
    }
    if BOOL_INI_KEYS.contains(&ini_key) {
        return match trimmed.to_ascii_lowercase().as_str() {
            "true" => "true".to_string(),
            "false" => "false".to_string(),
            other => other.to_string(),
        };
    }
    trimmed.to_string()
}

/// Reads the active `PalWorldSettings.ini`. Known keys become explicit fields +
/// reverse-mapped `env_vars` (for the settings UI); unmapped keys are ignored
/// here but preserved on disk by `build_palworld_settings_content`.
pub fn parse_server_config_from_ini(install_path: &str) -> ImportedServerConfig {
    let ini_path = config_dir(install_path).join("PalWorldSettings.ini");
    let pairs = parse_option_settings_ini(&ini_path).unwrap_or_default();

    let get = |wanted: &str| -> Option<&str> {
        pairs
            .iter()
            .find(|(key, _)| key == wanted)
            .map(|(_, value)| value.as_str())
    };
    let unquote = |value: Option<&str>| value.unwrap_or("").trim_matches('"').to_string();
    let parse_i64 = |value: Option<&str>, fallback: i64| -> i64 {
        value
            .and_then(|v| v.trim().parse::<i64>().ok())
            .unwrap_or(fallback)
    };

    let mut env_vars = serde_json::Map::new();
    for (ini_key, value) in &pairs {
        if let Some(env_key) = ini_to_env_key(ini_key) {
            env_vars.insert(
                env_key.to_string(),
                serde_json::Value::String(ini_value_to_env(ini_key, value)),
            );
        }
    }

    ImportedServerConfig {
        server_name: unquote(get("ServerName")),
        server_description: unquote(get("ServerDescription")),
        server_password: unquote(get("ServerPassword")),
        admin_password: unquote(get("AdminPassword")),
        max_players: parse_i64(get("ServerPlayerMaxNum"), 16),
        game_port: parse_i64(get("PublicPort"), 8211),
        rest_api_port: parse_i64(get("RESTAPIPort"), 8212),
        env_vars,
    }
}

fn upsert(defaults: &mut Vec<(String, String)>, key: &str, value: String) {
    if let Some(existing) = defaults
        .iter_mut()
        .find(|(existing_key, _)| existing_key == key)
    {
        existing.1 = value;
    } else {
        defaults.push((key.to_string(), value));
    }
}

/// Precedence: active ini, else shipped defaults, else hardcoded defaults; then
/// mapped `env_vars`, then the explicit server fields, which always win.
pub fn build_palworld_settings_content(record: &ServerRecord) -> String {
    let active_ini = config_dir(&record.install_path).join("PalWorldSettings.ini");
    let default_ini = Path::new(&record.install_path).join("DefaultPalWorldSettings.ini");
    let mut settings = parse_option_settings_ini(&active_ini)
        .or_else(|| parse_option_settings_ini(&default_ini))
        .unwrap_or_else(hardcoded_defaults);

    for (env_key, env_value) in record.env_vars.0.iter() {
        if is_docker_only_key(env_key) {
            continue;
        }
        let Some(ini_key) = env_to_ini_key(env_key) else {
            continue;
        };
        let value_text = crate::services::python_str(env_value);
        if value_text.is_empty() {
            continue;
        }
        upsert(
            &mut settings,
            ini_key,
            format_ini_value(ini_key, &value_text),
        );
    }

    upsert(
        &mut settings,
        "ServerName",
        format!("\"{}\"", record.server_name),
    );
    upsert(
        &mut settings,
        "ServerDescription",
        format!("\"{}\"", record.server_description),
    );
    upsert(
        &mut settings,
        "AdminPassword",
        format!("\"{}\"", record.admin_password),
    );
    upsert(
        &mut settings,
        "ServerPlayerMaxNum",
        record.max_players.to_string(),
    );
    upsert(&mut settings, "PublicPort", record.game_port.to_string());
    upsert(&mut settings, "RESTAPIEnabled", "True".to_string());
    upsert(
        &mut settings,
        "RESTAPIPort",
        record.rest_api_port.to_string(),
    );
    if record.server_password.is_empty() {
        upsert(&mut settings, "ServerPassword", "\"\"".to_string());
    } else {
        upsert(
            &mut settings,
            "ServerPassword",
            format!("\"{}\"", record.server_password),
        );
    }

    let pairs = settings
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("[/Script/Pal.PalGameWorldSettings]\nOptionSettings=({pairs})\n")
}

pub fn config_dir(install_path: &str) -> PathBuf {
    Path::new(install_path)
        .join("Pal")
        .join("Saved")
        .join("Config")
        .join("WindowsServer")
}

pub fn write_palworld_settings(record: &ServerRecord) -> std::io::Result<()> {
    let directory = config_dir(&record.install_path);
    std::fs::create_dir_all(&directory)?;
    std::fs::write(
        directory.join("PalWorldSettings.ini"),
        build_palworld_settings_content(record),
    )
}

/// Stock OptionSettings for a vanilla server, used when the install has no
/// readable DefaultPalWorldSettings.ini.
fn hardcoded_defaults() -> Vec<(String, String)> {
    [
        ("Difficulty", "None"),
        ("RandomizerType", "None"),
        ("RandomizerSeed", "\"\""),
        ("bIsRandomizerPalLevelRandom", "False"),
        ("DayTimeSpeedRate", "1.000000"),
        ("NightTimeSpeedRate", "1.000000"),
        ("ExpRate", "1.000000"),
        ("PalCaptureRate", "1.000000"),
        ("PalSpawnNumRate", "1.000000"),
        ("PalDamageRateAttack", "1.000000"),
        ("PalDamageRateDefense", "1.000000"),
        ("PlayerDamageRateAttack", "1.000000"),
        ("PlayerDamageRateDefense", "1.000000"),
        ("PlayerStomachDecreaceRate", "1.000000"),
        ("PlayerStaminaDecreaceRate", "1.000000"),
        ("PlayerAutoHPRegeneRate", "1.000000"),
        ("PlayerAutoHpRegeneRateInSleep", "1.000000"),
        ("PalStomachDecreaceRate", "1.000000"),
        ("PalStaminaDecreaceRate", "1.000000"),
        ("PalAutoHPRegeneRate", "1.000000"),
        ("PalAutoHpRegeneRateInSleep", "1.000000"),
        ("BuildObjectHpRate", "1.000000"),
        ("BuildObjectDamageRate", "1.000000"),
        ("BuildObjectDeteriorationDamageRate", "1.000000"),
        ("CollectionDropRate", "1.000000"),
        ("CollectionObjectHpRate", "1.000000"),
        ("CollectionObjectRespawnSpeedRate", "1.000000"),
        ("EnemyDropItemRate", "1.000000"),
        ("DeathPenalty", "All"),
        ("bEnablePlayerToPlayerDamage", "False"),
        ("bEnableFriendlyFire", "False"),
        ("bEnableInvaderEnemy", "True"),
        ("bActiveUNKO", "False"),
        ("bEnableAimAssistPad", "True"),
        ("bEnableAimAssistKeyboard", "False"),
        ("DropItemMaxNum", "3000"),
        ("DropItemMaxNum_UNKO", "100"),
        ("BaseCampMaxNum", "128"),
        ("BaseCampWorkerMaxNum", "15"),
        ("DropItemAliveMaxHours", "1.000000"),
        ("bAutoResetGuildNoOnlinePlayers", "False"),
        ("AutoResetGuildTimeNoOnlinePlayers", "72.000000"),
        ("GuildPlayerMaxNum", "20"),
        ("BaseCampMaxNumInGuild", "4"),
        ("PalEggDefaultHatchingTime", "72.000000"),
        ("WorkSpeedRate", "1.000000"),
        ("AutoSaveSpan", "30.000000"),
        ("bIsMultiplay", "False"),
        ("bIsPvP", "False"),
        ("bHardcore", "False"),
        ("bPalLost", "False"),
        ("bCharacterRecreateInHardcore", "False"),
        ("bCanPickupOtherGuildDeathPenaltyDrop", "False"),
        ("bEnableNonLoginPenalty", "True"),
        ("bEnableFastTravel", "True"),
        ("bEnableFastTravelOnlyBaseCamp", "False"),
        ("bIsStartLocationSelectByMap", "True"),
        ("bExistPlayerAfterLogout", "False"),
        ("bEnableDefenseOtherGuildPlayer", "False"),
        ("bInvisibleOtherGuildBaseCampAreaFX", "False"),
        ("bBuildAreaLimit", "False"),
        ("ItemWeightRate", "1.000000"),
        ("CoopPlayerMaxNum", "4"),
        ("ServerPlayerMaxNum", "32"),
        ("ServerName", "\"Default Palworld Server\""),
        ("ServerDescription", "\"\""),
        ("AdminPassword", "\"\""),
        ("ServerPassword", "\"\""),
        ("bAllowClientMod", "True"),
        ("PublicPort", "8211"),
        ("PublicIP", "\"\""),
        ("RCONEnabled", "False"),
        ("RCONPort", "25575"),
        ("Region", "\"\""),
        ("bUseAuth", "True"),
        (
            "BanListURL",
            "\"https://b.palworldgame.com/api/banlist.txt\"",
        ),
        ("RESTAPIEnabled", "False"),
        ("RESTAPIPort", "8212"),
        ("bShowPlayerList", "False"),
        ("ChatPostLimitPerMinute", "30"),
        ("CrossplayPlatforms", "(Steam,Xbox,PS5,Mac)"),
        ("bIsUseBackupSaveData", "True"),
        ("LogFormatType", "Text"),
        ("bIsShowJoinLeftMessage", "True"),
        ("SupplyDropSpan", "180"),
        ("EnablePredatorBossPal", "True"),
        ("MaxBuildingLimitNum", "0"),
        ("ServerReplicatePawnCullDistance", "15000.000000"),
        ("bAllowGlobalPalboxExport", "True"),
        ("bAllowGlobalPalboxImport", "False"),
        ("EquipmentDurabilityDamageRate", "1.000000"),
        ("ItemContainerForceMarkDirtyInterval", "1.000000"),
        ("ItemCorruptionMultiplier", "1.000000"),
        ("DenyTechnologyList", ""),
        ("GuildRejoinCooldownMinutes", "0"),
        ("BlockRespawnTime", "5.000000"),
        ("RespawnPenaltyDurationThreshold", "0.000000"),
        ("RespawnPenaltyTimeScale", "2.000000"),
        ("bDisplayPvPItemNumOnWorldMap_BaseCamp", "False"),
        ("bDisplayPvPItemNumOnWorldMap_Player", "False"),
        (
            "AdditionalDropItemWhenPlayerKillingInPvPMode",
            "\"PlayerDropItem\"",
        ),
        ("AdditionalDropItemNumWhenPlayerKillingInPvPMode", "1"),
        ("bAdditionalDropItemWhenPlayerKillingInPvPMode", "False"),
        ("bAllowEnhanceStat_Health", "True"),
        ("bAllowEnhanceStat_Attack", "True"),
        ("bAllowEnhanceStat_Stamina", "True"),
        ("bAllowEnhanceStat_Weight", "True"),
        ("bAllowEnhanceStat_WorkSpeed", "True"),
    ]
    .iter()
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native_record(install_path: &str) -> psp_db::servers::ServerRecord {
        let mut record = crate::services::docker::test_support::docker_record();
        record.server_type = "native".to_string();
        record.install_path = install_path.to_string();
        record.server_name = "My Native Server".to_string();
        record.server_description = "A server".to_string();
        record.admin_password = "hunter2".to_string();
        record.server_password = "guest".to_string();
        record.max_players = 24;
        record.game_port = 8311;
        record.rest_api_port = 8312;
        record.env_vars = sqlx::types::Json(serde_json::Map::new());
        record
    }

    #[test]
    fn path_helpers_match_python_layout() {
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(
            saves_path("/srv/pal"),
            format!("/srv/pal{sep}Pal{sep}Saved")
        );
        assert_eq!(
            mods_path("/srv/pal"),
            format!("/srv/pal{sep}Pal{sep}Binaries{sep}Win64{sep}Mods")
        );
        assert_eq!(
            logicmods_path("/srv/pal"),
            format!("/srv/pal{sep}Pal{sep}Content{sep}Paks{sep}LogicMods")
        );
        assert_eq!(
            nativemods_path("/srv/pal"),
            format!("/srv/pal{sep}Pal{sep}Binaries{sep}Win64{sep}NativeMods")
        );
    }

    #[test]
    fn env_to_ini_key_maps_known_keys_and_rejects_unknown() {
        assert_eq!(env_to_ini_key("EXP_RATE"), Some("ExpRate"));
        assert_eq!(env_to_ini_key("SERVER_NAME"), Some("ServerName"));
        assert_eq!(
            env_to_ini_key("CROSSPLAY_PLATFORMS"),
            Some("CrossplayPlatforms")
        );
        assert_eq!(env_to_ini_key("TOTALLY_UNKNOWN"), None);
        assert!(is_docker_only_key("UPDATE_ON_BOOT"));
        assert!(!is_docker_only_key("EXP_RATE"));
    }

    #[test]
    fn format_ini_value_quotes_strings_and_normalizes_bools() {
        assert_eq!(format_ini_value("ServerName", "My Server"), "\"My Server\"");
        assert_eq!(format_ini_value("ServerName", "\"Already\""), "\"Already\"");
        assert_eq!(format_ini_value("bIsPvP", "true"), "True");
        assert_eq!(format_ini_value("bIsPvP", "0"), "False");
        assert_eq!(format_ini_value("ExpRate", "2.5"), "2.5");
        assert_eq!(
            format_ini_value("CrossplayPlatforms", "(Steam,Xbox)"),
            "(Steam,Xbox)"
        );
    }

    #[test]
    fn split_option_settings_respects_quotes_and_parens() {
        let split =
            split_option_settings("A=1,B=\"x, y\",CrossplayPlatforms=(Steam,Xbox,PS5,Mac),C=2");
        assert_eq!(
            split,
            vec![
                "A=1",
                "B=\"x, y\"",
                "CrossplayPlatforms=(Steam,Xbox,PS5,Mac)",
                "C=2"
            ]
        );
    }

    #[test]
    fn build_content_uses_default_ini_when_present_and_applies_overrides() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        std::fs::write(
            scratch.path().join("DefaultPalWorldSettings.ini"),
            "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(Difficulty=None,ExpRate=1.000000,ServerName=\"Default Palworld Server\")\n",
        )
        .unwrap();
        let mut record = native_record(&install);
        record
            .env_vars
            .0
            .insert("EXP_RATE".to_string(), serde_json::json!("3.0"));
        record
            .env_vars
            .0
            .insert("UPDATE_ON_BOOT".to_string(), serde_json::json!("true")); // docker-only, skipped
        let content = build_palworld_settings_content(&record);
        assert!(content.starts_with("[/Script/Pal.PalGameWorldSettings]\nOptionSettings=("));
        assert!(content.contains("ExpRate=3.0"));
        assert!(content.contains("ServerName=\"My Native Server\""));
        assert!(content.contains("ServerDescription=\"A server\""));
        assert!(content.contains("AdminPassword=\"hunter2\""));
        assert!(content.contains("ServerPlayerMaxNum=24"));
        assert!(content.contains("PublicPort=8311"));
        assert!(content.contains("RESTAPIEnabled=True"));
        assert!(content.contains("RESTAPIPort=8312"));
        assert!(content.contains("ServerPassword=\"guest\""));
        assert!(!content.contains("UpdateOnBoot"));
    }

    #[test]
    fn build_content_preserves_unknown_keys_from_active_ini() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        // Active ini contains a key PSP does not map.
        let cfg = config_dir(&install);
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("PalWorldSettings.ini"),
            "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(ServerName=\"Old\",ExpRate=1.000000,MyCustomKey=42)\n",
        )
        .unwrap();

        let record = native_record(&install); // existing test helper in this module
        let content = build_palworld_settings_content(&record);
        // Unmapped key survives...
        assert!(content.contains("MyCustomKey=42"));
        // ...while PSP-owned explicit fields still override.
        assert!(content.contains("ServerName=\"My Native Server\""));
    }

    #[test]
    fn build_content_falls_back_to_hardcoded_defaults() {
        let scratch = tempfile::tempdir().unwrap();
        let mut record = native_record(&scratch.path().to_string_lossy());
        record.server_password = String::new();
        let content = build_palworld_settings_content(&record);
        // Sentinel values from the hardcoded fallback
        assert!(content.contains("Difficulty=None"));
        assert!(content.contains("BanListURL=\"https://b.palworldgame.com/api/banlist.txt\""));
        assert!(content.contains("CrossplayPlatforms=(Steam,Xbox,PS5,Mac)"));
        assert!(content.contains("ServerPassword=\"\""));
    }

    #[test]
    fn ini_to_env_key_inverts_known_keys() {
        assert_eq!(ini_to_env_key("ExpRate"), Some("EXP_RATE"));
        assert_eq!(ini_to_env_key("ServerName"), Some("SERVER_NAME"));
        assert_eq!(ini_to_env_key("TotallyUnknown"), None);
    }

    #[test]
    fn parse_server_config_reads_explicit_fields_ports_and_env() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        let cfg = config_dir(&install);
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("PalWorldSettings.ini"),
            "[/Script/Pal.PalGameWorldSettings]\nOptionSettings=(ServerName=\"My Imported\",ServerDescription=\"Desc\",AdminPassword=\"pw\",ServerPassword=\"guest\",ServerPlayerMaxNum=24,PublicPort=9911,RESTAPIPort=9912,ExpRate=3.000000,bIsPvP=True,MyCustomKey=42)\n",
        )
        .unwrap();

        let parsed = parse_server_config_from_ini(&install);
        assert_eq!(parsed.server_name, "My Imported");
        assert_eq!(parsed.server_description, "Desc");
        assert_eq!(parsed.admin_password, "pw");
        assert_eq!(parsed.server_password, "guest");
        assert_eq!(parsed.max_players, 24);
        assert_eq!(parsed.game_port, 9911);
        assert_eq!(parsed.rest_api_port, 9912);
        // known keys reverse-mapped and normalized (quotes stripped, bools lowercased)
        assert_eq!(parsed.env_vars.get("EXP_RATE").unwrap(), "3.000000");
        assert_eq!(parsed.env_vars.get("IS_PVP").unwrap(), "true");
        // unknown ini key is not surfaced as an env var
        assert!(!parsed.env_vars.values().any(|v| v == "42"));
    }

    #[test]
    fn parse_server_config_uses_defaults_when_ini_missing() {
        let scratch = tempfile::tempdir().unwrap();
        let parsed = parse_server_config_from_ini(&scratch.path().to_string_lossy());
        assert_eq!(parsed.game_port, 8211);
        assert_eq!(parsed.rest_api_port, 8212);
        assert_eq!(parsed.max_players, 16);
        assert!(parsed.server_name.is_empty());
    }

    #[test]
    fn write_palworld_settings_creates_config_file() {
        let scratch = tempfile::tempdir().unwrap();
        let record = native_record(&scratch.path().to_string_lossy());
        write_palworld_settings(&record).unwrap();
        let ini_path = scratch
            .path()
            .join("Pal")
            .join("Saved")
            .join("Config")
            .join("WindowsServer")
            .join("PalWorldSettings.ini");
        let contents = std::fs::read_to_string(ini_path).unwrap();
        assert!(contents.ends_with(")\n"));
    }
}
