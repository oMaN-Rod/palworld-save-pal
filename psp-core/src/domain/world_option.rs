//! `WorldOption.sav` settings access.
//!
//! `WORLD_OPTION_SETTINGS` is the single source of truth for every setting's GVAS
//! type and its write schema. The table was generated from the real testdata corpus
//! (7 files, zero tag conflicts) rather than hand-written -- see the spec's Risks
//! section. `world_option_table_matches_corpus` in `world_option_corpus.rs`
//! is what keeps it honest when Palworld ships new settings.
//!
//! The file is SPARSE: real saves carry anywhere from 87 to 119 of these keys, and
//! the `Version` property (always 101) does not discriminate. Presence is read from
//! the data, never inferred.

use crate::ue::{PropertyTagDataPartial, PropertyTagPartial, PropertyType};

use crate::error::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WoKind {
    Bool,
    Int,
    Float,
    Str,
    Name,
    /// Carries the enum name recorded in the tag, e.g. "EPalOptionWorldDifficulty".
    Enum(&'static str),
    /// `CrossplayPlatforms`. Takes NO enum name: the corpus records the element tag
    /// as `Enum("", None)`. The fully-qualified `EPalAllowConnectPlatform::*` form
    /// appears only in the values.
    EnumArray,
    /// `DenyTechnologyList`.
    NameArray,
}

/// Root property holding the settings struct.
pub const OPTION_WORLD_DATA: &str = "OptionWorldData";
/// The settings struct within it.
pub const SETTINGS: &str = "Settings";

/// Dotted schema path uesave's writer looks a property up by.
pub fn settings_schema_path(key: &str) -> String {
    format!("{OPTION_WORLD_DATA}.{SETTINGS}.{key}")
}

pub fn kind_for(key: &str) -> Option<WoKind> {
    WORLD_OPTION_SETTINGS
        .iter()
        .find_map(|(k, kind)| (*k == key).then_some(*kind))
}

pub fn tag_for(kind: WoKind) -> PropertyTagPartial {
    let data = match kind {
        WoKind::Bool => PropertyTagDataPartial::Other(PropertyType::BoolProperty),
        WoKind::Int => PropertyTagDataPartial::Other(PropertyType::IntProperty),
        WoKind::Float => PropertyTagDataPartial::Other(PropertyType::FloatProperty),
        WoKind::Str => PropertyTagDataPartial::Other(PropertyType::StrProperty),
        WoKind::Name => PropertyTagDataPartial::Other(PropertyType::NameProperty),
        WoKind::Enum(name) => PropertyTagDataPartial::Enum(name.to_string(), None),
        WoKind::EnumArray => PropertyTagDataPartial::Array(Box::new(
            PropertyTagDataPartial::Enum(String::new(), None),
        )),
        WoKind::NameArray => PropertyTagDataPartial::Array(Box::new(
            PropertyTagDataPartial::Other(PropertyType::NameProperty),
        )),
    };
    PropertyTagPartial { id: None, data }
}

/// Records a write schema for every known setting, whether or not this file carried
/// it. Called once at parse (`SaveSession::load`), never at a mutation site: that is
/// what lets a later edit add a setting the source file omitted. `props::ensure_schema`
/// never overwrites a schema read from the real save, so this is safe to call on any
/// file.
pub fn ensure_world_option_schemas(save: &mut crate::ue::Save) {
    for (key, kind) in WORLD_OPTION_SETTINGS {
        crate::props::ensure_schema(save, settings_schema_path(key), tag_for(*kind));
    }
}

pub const WORLD_OPTION_SETTINGS: &[(&str, WoKind)] = &[
    ("AdditionalDropItemNumWhenPlayerKillingInPvPMode", WoKind::Int),
    ("AdditionalDropItemWhenPlayerKillingInPvPMode", WoKind::Name),
    ("AdminPassword", WoKind::Str),
    ("AutoResetGuildTimeNoOnlinePlayers", WoKind::Float),
    ("AutoTransferMasterCheckIntervalSeconds", WoKind::Float),
    ("AutoTransferMasterThresholdDays", WoKind::Int),
    ("BanListURL", WoKind::Str),
    ("BaseCampMaxNum", WoKind::Int),
    ("BaseCampMaxNumInGuild", WoKind::Int),
    ("BaseCampWorkerMaxNum", WoKind::Int),
    ("BlockRespawnTime", WoKind::Float),
    ("BuildObjectDamageRate", WoKind::Float),
    ("BuildObjectDeteriorationDamageRate", WoKind::Float),
    ("BuildObjectHpRate", WoKind::Float),
    ("BuildingNameDisplayCacheTTLSeconds", WoKind::Int),
    ("ChatPostLimitPerMinute", WoKind::Int),
    ("CollectionDropRate", WoKind::Float),
    ("CollectionObjectHpRate", WoKind::Float),
    ("CollectionObjectRespawnSpeedRate", WoKind::Float),
    ("CoopPlayerMaxNum", WoKind::Int),
    ("CrossplayPlatforms", WoKind::EnumArray),
    ("DayTimeSpeedRate", WoKind::Float),
    ("DeathPenalty", WoKind::Enum("EPalOptionWorldDeathPenalty")),
    ("DenyTechnologyList", WoKind::NameArray),
    ("Difficulty", WoKind::Enum("EPalOptionWorldDifficulty")),
    ("DropItemAliveMaxHours", WoKind::Float),
    ("DropItemMaxNum", WoKind::Int),
    ("DropItemMaxNum_UNKO", WoKind::Int),
    ("EnablePredatorBossPal", WoKind::Bool),
    ("EnemyDropItemRate", WoKind::Float),
    ("EquipmentDurabilityDamageRate", WoKind::Float),
    ("ExpRate", WoKind::Float),
    ("GuildPlayerMaxNum", WoKind::Int),
    ("GuildRejoinCooldownMinutes", WoKind::Int),
    ("ItemContainerForceMarkDirtyInterval", WoKind::Float),
    ("ItemCorruptionMultiplier", WoKind::Float),
    ("ItemWeightRate", WoKind::Float),
    ("LogFormatType", WoKind::Enum("EPalLogFormatType")),
    ("MaxBuildingLimitNum", WoKind::Int),
    ("MaxGuildsPerFrame", WoKind::Int),
    ("MonsterFarmActionSpeedRate", WoKind::Float),
    ("NightTimeSpeedRate", WoKind::Float),
    ("PalAutoHPRegeneRate", WoKind::Float),
    ("PalAutoHpRegeneRateInSleep", WoKind::Float),
    ("PalCaptureRate", WoKind::Float),
    ("PalDamageRateAttack", WoKind::Float),
    ("PalDamageRateDefense", WoKind::Float),
    ("PalEggDefaultHatchingTime", WoKind::Float),
    ("PalSpawnNumRate", WoKind::Float),
    ("PalStaminaDecreaceRate", WoKind::Float),
    ("PalStomachDecreaceRate", WoKind::Float),
    ("PhysicsActiveDropItemMaxNum", WoKind::Int),
    ("PlayerAutoHPRegeneRate", WoKind::Float),
    ("PlayerAutoHpRegeneRateInSleep", WoKind::Float),
    ("PlayerDamageRateAttack", WoKind::Float),
    ("PlayerDamageRateDefense", WoKind::Float),
    ("PlayerDataPalStorageUpdateCheckTickInterval", WoKind::Float),
    ("PlayerStaminaDecreaceRate", WoKind::Float),
    ("PlayerStomachDecreaceRate", WoKind::Float),
    ("PublicIP", WoKind::Str),
    ("PublicPort", WoKind::Int),
    ("RCONEnabled", WoKind::Bool),
    ("RCONPort", WoKind::Int),
    ("RESTAPIEnabled", WoKind::Bool),
    ("RESTAPIPort", WoKind::Int),
    ("RandomizerSeed", WoKind::Str),
    ("RandomizerType", WoKind::Enum("EPalRandomizerType")),
    ("Region", WoKind::Str),
    ("RespawnPenaltyDurationThreshold", WoKind::Float),
    ("RespawnPenaltyTimeScale", WoKind::Float),
    ("ServerDescription", WoKind::Str),
    ("ServerName", WoKind::Str),
    ("ServerPassword", WoKind::Str),
    ("ServerPlayerMaxNum", WoKind::Int),
    ("ServerReplicatePawnCullDistance", WoKind::Float),
    ("SupplyDropSpan", WoKind::Int),
    ("VoiceChatMaxVolumeDistance", WoKind::Float),
    ("VoiceChatZeroVolumeDistance", WoKind::Float),
    ("WorkSpeedRate", WoKind::Float),
    ("autoSaveSpan", WoKind::Float),
    ("bActiveUNKO", WoKind::Bool),
    ("bAdditionalDropItemWhenPlayerKillingInPvPMode", WoKind::Bool),
    ("bAllowClientMod", WoKind::Bool),
    ("bAllowEnhanceStat_Attack", WoKind::Bool),
    ("bAllowEnhanceStat_Health", WoKind::Bool),
    ("bAllowEnhanceStat_Stamina", WoKind::Bool),
    ("bAllowEnhanceStat_Weight", WoKind::Bool),
    ("bAllowEnhanceStat_WorkSpeed", WoKind::Bool),
    ("bAllowGlobalPalboxExport", WoKind::Bool),
    ("bAllowGlobalPalboxImport", WoKind::Bool),
    ("bAutoResetGuildNoOnlinePlayers", WoKind::Bool),
    ("bBuildAreaLimit", WoKind::Bool),
    ("bCanPickupOtherGuildDeathPenaltyDrop", WoKind::Bool),
    ("bCharacterRecreateInHardcore", WoKind::Bool),
    ("bDisplayPvPItemNumOnWorldMap_BaseCamp", WoKind::Bool),
    ("bDisplayPvPItemNumOnWorldMap_Player", WoKind::Bool),
    ("bEnableAimAssistKeyboard", WoKind::Bool),
    ("bEnableAimAssistPad", WoKind::Bool),
    ("bEnableBuildingPlayerUIdDisplay", WoKind::Bool),
    ("bEnableDefenseOtherGuildPlayer", WoKind::Bool),
    ("bEnableFastTravel", WoKind::Bool),
    ("bEnableFastTravelOnlyBaseCamp", WoKind::Bool),
    ("bEnableFriendlyFire", WoKind::Bool),
    ("bEnableInvaderEnemy", WoKind::Bool),
    ("bEnableNonLoginPenalty", WoKind::Bool),
    ("bEnablePlayerToPlayerDamage", WoKind::Bool),
    ("bEnableVoiceChat", WoKind::Bool),
    ("bExistPlayerAfterLogout", WoKind::Bool),
    ("bHardcore", WoKind::Bool),
    ("bInvisibleOtherGuildBaseCampAreaFX", WoKind::Bool),
    ("bIsMultiplay", WoKind::Bool),
    ("bIsPvP", WoKind::Bool),
    ("bIsRandomizerPalLevelRandom", WoKind::Bool),
    ("bIsShowJoinLeftMessage", WoKind::Bool),
    ("bIsStartLocationSelectByMap", WoKind::Bool),
    ("bIsUseBackupSaveData", WoKind::Bool),
    ("bPalLost", WoKind::Bool),
    ("bShowPlayerList", WoKind::Bool),
    ("bUseAuth", WoKind::Bool),
];

#[derive(Debug, Clone)]
pub struct WorldOptionEntry {
    pub key: String,
    pub kind: WoKind,
    pub value: serde_json::Value,
}

/// The `Settings` property bag, or `None` on a save that isn't a WorldOption.
fn settings_properties(save: &crate::ue::Save) -> Option<&crate::ue::Properties> {
    crate::props::get(&save.root.properties, &[OPTION_WORLD_DATA, SETTINGS])
        .and_then(crate::props::struct_props)
}

fn settings_properties_mut(save: &mut crate::ue::Save) -> Option<&mut crate::ue::Properties> {
    crate::props::get_mut(&mut save.root.properties, &[OPTION_WORLD_DATA, SETTINGS])
        .and_then(crate::props::struct_props_mut)
}

/// Encodes one property as wire JSON. Returns `None` when the stored property's
/// shape disagrees with the table -- an untrusted save is allowed to be wrong.
fn encode_value(kind: WoKind, property: &crate::ue::Property) -> Option<serde_json::Value> {
    Some(match kind {
        WoKind::Bool => serde_json::json!(crate::props::as_bool(property)?),
        WoKind::Int => serde_json::json!(crate::props::as_i32(property)?),
        WoKind::Float => serde_json::json!(crate::props::as_f32(property)?),
        WoKind::Str | WoKind::Name => serde_json::json!(crate::props::as_str(property)?),
        WoKind::Enum(_) => serde_json::json!(crate::props::as_enum(property)?),
        WoKind::EnumArray => serde_json::json!(crate::props::enum_values(property)?),
        WoKind::NameArray => serde_json::json!(crate::props::name_values(property)?),
    })
}

/// Present keys only, in GVAS order. Keys absent from `WORLD_OPTION_SETTINGS` (a
/// future Palworld setting) are skipped here but left untouched in the tree, so they
/// round-trip on write rather than being dropped.
pub fn read_settings(save: &crate::ue::Save) -> Vec<WorldOptionEntry> {
    let Some(properties) = settings_properties(save) else {
        return Vec::new();
    };
    properties
        .into_iter()
        .filter_map(|(property_key, property)| {
            let key = property_key.1.as_str();
            let kind = kind_for(key)?;
            let value = encode_value(kind, property)?;
            Some(WorldOptionEntry {
                key: key.to_string(),
                kind,
                value,
            })
        })
        .collect()
}

/// The root `Version` property. Display-only; never written.
pub fn read_version(save: &crate::ue::Save) -> i32 {
    crate::props::get(&save.root.properties, &["Version"])
        .and_then(crate::props::as_i32)
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
pub struct WorldOptionPatch {
    pub key: String,
    pub value: serde_json::Value,
}

fn kind_error(key: &str, expected: &str) -> CoreError {
    CoreError::Parse(format!(
        "WorldOption setting '{key}' expects {expected}"
    ))
}

/// Decodes wire JSON into a `Property` per the table. Rejects anything the table
/// disagrees with, so a malformed client patch can never write a wrong-typed
/// property into a real save.
fn decode_value(key: &str, kind: WoKind, value: &serde_json::Value) -> Result<crate::ue::Property, CoreError> {
    Ok(match kind {
        WoKind::Bool => crate::props::bool_property(
            value.as_bool().ok_or_else(|| kind_error(key, "a boolean"))?,
        ),
        WoKind::Int => crate::props::int_property(
            value
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| kind_error(key, "a 32-bit integer"))?,
        ),
        WoKind::Float => crate::props::float_property(
            value.as_f64().ok_or_else(|| kind_error(key, "a number"))? as f32,
        ),
        WoKind::Str => crate::props::str_property(
            value.as_str().ok_or_else(|| kind_error(key, "a string"))?,
        ),
        WoKind::Name => crate::props::name_property(
            value.as_str().ok_or_else(|| kind_error(key, "a string"))?,
        ),
        WoKind::Enum(enum_name) => {
            let text = value.as_str().ok_or_else(|| kind_error(key, "a string"))?;
            // Fully-qualified only: a bare "Custom" would write a value the game
            // cannot read back.
            if !text.starts_with(&format!("{enum_name}::")) {
                return Err(kind_error(key, &format!("a fully-qualified {enum_name}:: variant")));
            }
            crate::props::enum_property(text)
        }
        WoKind::EnumArray => crate::props::enum_array_property(decode_string_array(key, value)?),
        WoKind::NameArray => crate::props::name_array_property(decode_string_array(key, value)?),
    })
}

fn decode_string_array(key: &str, value: &serde_json::Value) -> Result<Vec<String>, CoreError> {
    value
        .as_array()
        .ok_or_else(|| kind_error(key, "an array of strings"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| kind_error(key, "an array of strings"))
        })
        .collect()
}

/// Applies only the keys in `patch` -- the patch IS the minimal diff. Returns
/// whether anything actually changed; the caller uses that as its dirty flag, so a
/// no-op patch never triggers a rewrite of the user's file.
///
/// Adding a key the source file omitted is safe because `ensure_world_option_schemas`
/// primed every schema at parse.
pub fn apply_patch(save: &mut crate::ue::Save, patch: &[WorldOptionPatch]) -> Result<bool, CoreError> {
    if patch.is_empty() {
        return Ok(false);
    }

    // Decode everything before mutating, so a rejected entry leaves the save untouched.
    let mut decoded: Vec<(&str, crate::ue::Property)> = Vec::with_capacity(patch.len());
    for entry in patch {
        let kind = kind_for(&entry.key).ok_or_else(|| {
            CoreError::Parse(format!("Unknown WorldOption setting '{}'", entry.key))
        })?;
        decoded.push((entry.key.as_str(), decode_value(&entry.key, kind, &entry.value)?));
    }

    let properties = settings_properties_mut(save)
        .ok_or_else(|| CoreError::Parse("WorldOption OptionWorldData.Settings missing".into()))?;

    let mut dirty = false;
    for (key, property) in decoded {
        let unchanged = properties
            .0
            .get(&crate::ue::PropertyKey::from(key))
            .is_some_and(|existing| *existing == property);
        if unchanged {
            continue;
        }
        // IndexMap::insert replaces in place for a present key (preserving GVAS
        // order) and appends for a new one.
        properties.insert(key, property);
        dirty = true;
    }
    Ok(dirty)
}

impl WoKind {
    /// Lowercase wire tag. Must match `WoFieldKind` in
    /// `ui/src/lib/components/worldoption/worldOptionFields.ts`.
    pub fn wire_tag(self) -> &'static str {
        match self {
            WoKind::Bool => "bool",
            WoKind::Int => "int",
            WoKind::Float => "float",
            WoKind::Str => "str",
            WoKind::Name => "name",
            WoKind::Enum(_) => "enum",
            WoKind::EnumArray => "enum_array",
            WoKind::NameArray => "name_array",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_119_settings_with_expected_kind_histogram() {
        assert_eq!(WORLD_OPTION_SETTINGS.len(), 119);

        let count = |pred: fn(&WoKind) -> bool| {
            WORLD_OPTION_SETTINGS.iter().filter(|(_, k)| pred(k)).count()
        };
        assert_eq!(count(|k| matches!(k, WoKind::Bool)), 42, "Bool count");
        assert_eq!(count(|k| matches!(k, WoKind::Float)), 42, "Float count");
        assert_eq!(count(|k| matches!(k, WoKind::Int)), 20, "Int count");
        assert_eq!(count(|k| matches!(k, WoKind::Str)), 8, "Str count");
        assert_eq!(count(|k| matches!(k, WoKind::Enum(_))), 4, "Enum count");
        assert_eq!(count(|k| matches!(k, WoKind::Name)), 1, "Name count");
        assert_eq!(count(|k| matches!(k, WoKind::NameArray)), 1, "NameArray count");
        assert_eq!(count(|k| matches!(k, WoKind::EnumArray)), 1, "EnumArray count");
    }

    #[test]
    fn table_keys_are_unique() {
        let mut keys: Vec<&str> = WORLD_OPTION_SETTINGS.iter().map(|(k, _)| *k).collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), total, "duplicate key in WORLD_OPTION_SETTINGS");
    }

    #[test]
    fn settings_schema_path_is_flat_under_option_world_data() {
        assert_eq!(
            settings_schema_path("ExpRate"),
            "OptionWorldData.Settings.ExpRate"
        );
    }

    #[test]
    fn tag_for_enum_array_uses_empty_enum_name() {
        // Verified against the real corpus: CrossplayPlatforms' element tag is
        // Enum("", None) -- NOT "EPalAllowConnectPlatform".
        let tag = tag_for(WoKind::EnumArray);
        match tag.data {
            crate::ue::PropertyTagDataPartial::Array(inner) => match *inner {
                crate::ue::PropertyTagDataPartial::Enum(name, second) => {
                    assert_eq!(name, "");
                    assert_eq!(second, None);
                }
                other => panic!("expected Array(Enum), got {other:?}"),
            },
            other => panic!("expected Array, got {other:?}"),
        }
    }

    #[test]
    fn tag_for_name_array_is_array_of_name_property() {
        let tag = tag_for(WoKind::NameArray);
        match tag.data {
            crate::ue::PropertyTagDataPartial::Array(inner) => assert!(matches!(
                *inner,
                crate::ue::PropertyTagDataPartial::Other(crate::ue::PropertyType::NameProperty)
            )),
            other => panic!("expected Array, got {other:?}"),
        }
    }

    #[test]
    fn tag_for_enum_carries_name_and_none_second_field() {
        let tag = tag_for(WoKind::Enum("EPalOptionWorldDifficulty"));
        match tag.data {
            crate::ue::PropertyTagDataPartial::Enum(name, second) => {
                assert_eq!(name, "EPalOptionWorldDifficulty");
                assert_eq!(second, None);
            }
            other => panic!("expected Enum, got {other:?}"),
        }
    }

    #[test]
    fn kind_for_resolves_known_keys_and_rejects_unknown() {
        assert!(matches!(kind_for("ExpRate"), Some(WoKind::Float)));
        assert!(matches!(kind_for("bIsPvP"), Some(WoKind::Bool)));
        assert!(matches!(kind_for("DenyTechnologyList"), Some(WoKind::NameArray)));
        assert!(matches!(kind_for("CrossplayPlatforms"), Some(WoKind::EnumArray)));
        assert!(kind_for("NoSuchSetting").is_none());
    }

    fn settings_save(entries: Vec<(&str, crate::ue::Property)>) -> crate::ue::Save {
        let mut settings = crate::ue::Properties::default();
        for (key, value) in entries {
            settings.insert(key, value);
        }
        let mut owd = crate::ue::Properties::default();
        owd.insert(SETTINGS, crate::ue::Property::Struct(crate::ue::StructValue::Struct(settings)));
        let mut root = crate::ue::Properties::default();
        root.insert("Version", crate::props::int_property(101));
        root.insert(
            OPTION_WORLD_DATA,
            crate::ue::Property::Struct(crate::ue::StructValue::Struct(owd)),
        );
        crate::ue::Save {
            header: crate::ue::Header {
                magic: 0,
                save_game_version: 0,
                package_version: crate::ue::PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: crate::ue::PropertySchemas::default(),
            root: crate::ue::Root {
                save_game_type: String::new(),
                properties: root,
            },
            extra: Vec::new(),
        }
    }

    #[test]
    fn read_settings_returns_only_present_keys_in_gvas_order() {
        let save = settings_save(vec![
            ("ExpRate", crate::props::float_property(20.0)),
            ("bIsPvP", crate::props::bool_property(true)),
            ("ServerName", crate::props::str_property("My Server")),
        ]);

        let entries = read_settings(&save);

        assert_eq!(entries.len(), 3, "absent keys must not be synthesized");
        assert_eq!(entries[0].key, "ExpRate");
        assert_eq!(entries[0].value, serde_json::json!(20.0));
        assert_eq!(entries[1].key, "bIsPvP");
        assert_eq!(entries[1].value, serde_json::json!(true));
        assert_eq!(entries[2].key, "ServerName");
        assert_eq!(entries[2].value, serde_json::json!("My Server"));
    }

    #[test]
    fn read_settings_encodes_enums_fully_qualified() {
        let save = settings_save(vec![(
            "Difficulty",
            crate::props::enum_property("EPalOptionWorldDifficulty::Custom"),
        )]);

        let entries = read_settings(&save);

        assert_eq!(entries[0].value, serde_json::json!("EPalOptionWorldDifficulty::Custom"));
    }

    #[test]
    fn read_settings_encodes_arrays_as_string_lists() {
        let save = settings_save(vec![
            (
                "CrossplayPlatforms",
                crate::props::enum_array_property(vec![
                    "EPalAllowConnectPlatform::Steam".to_string(),
                    "EPalAllowConnectPlatform::Xbox".to_string(),
                ]),
            ),
            (
                "DenyTechnologyList",
                crate::props::name_array_property(vec!["AIcore".to_string()]),
            ),
        ]);

        let entries = read_settings(&save);

        assert_eq!(
            entries[0].value,
            serde_json::json!(["EPalAllowConnectPlatform::Steam", "EPalAllowConnectPlatform::Xbox"])
        );
        assert_eq!(entries[1].value, serde_json::json!(["AIcore"]));
    }

    #[test]
    fn read_settings_skips_keys_absent_from_the_table() {
        let save = settings_save(vec![
            ("ExpRate", crate::props::float_property(1.0)),
            ("SomeFutureSetting", crate::props::bool_property(true)),
        ]);

        let entries = read_settings(&save);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "ExpRate");
    }

    #[test]
    fn read_settings_on_a_save_without_option_world_data_is_empty() {
        let save = settings_save(vec![]);
        assert!(read_settings(&save).is_empty());
    }

    #[test]
    fn read_version_reads_the_root_version_property() {
        let save = settings_save(vec![]);
        assert_eq!(read_version(&save), 101);
    }

    fn patch(key: &str, value: serde_json::Value) -> WorldOptionPatch {
        WorldOptionPatch { key: key.to_string(), value }
    }

    #[test]
    fn apply_patch_updates_a_present_key_in_place_without_reordering() {
        let mut save = settings_save(vec![
            ("ExpRate", crate::props::float_property(1.0)),
            ("bIsPvP", crate::props::bool_property(false)),
        ]);

        let dirty = apply_patch(&mut save, &[patch("ExpRate", serde_json::json!(5.0))]).unwrap();

        assert!(dirty);
        let entries = read_settings(&save);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, "ExpRate", "position must be preserved");
        assert_eq!(entries[0].value, serde_json::json!(5.0));
        assert_eq!(entries[1].key, "bIsPvP");
    }

    #[test]
    fn apply_patch_adds_an_absent_key() {
        let mut save = settings_save(vec![("ExpRate", crate::props::float_property(1.0))]);

        let dirty = apply_patch(&mut save, &[patch("bEnableVoiceChat", serde_json::json!(true))]).unwrap();

        assert!(dirty);
        let entries = read_settings(&save);
        assert_eq!(entries.len(), 2, "absent key must be added, not ignored");
        assert_eq!(entries[1].key, "bEnableVoiceChat");
        assert_eq!(entries[1].value, serde_json::json!(true));
    }

    #[test]
    fn apply_empty_patch_is_not_dirty() {
        let mut save = settings_save(vec![("ExpRate", crate::props::float_property(1.0))]);
        assert!(!apply_patch(&mut save, &[]).unwrap());
    }

    #[test]
    fn apply_patch_writing_an_identical_value_is_not_dirty() {
        let mut save = settings_save(vec![("ExpRate", crate::props::float_property(1.0))]);
        // A no-op edit must not trigger a rewrite of the user's file.
        assert!(!apply_patch(&mut save, &[patch("ExpRate", serde_json::json!(1.0))]).unwrap());
    }

    #[test]
    fn apply_patch_rejects_unknown_key() {
        let mut save = settings_save(vec![]);
        let error = apply_patch(&mut save, &[patch("NoSuchSetting", serde_json::json!(1))]).unwrap_err();
        assert!(format!("{error}").contains("NoSuchSetting"));
    }

    #[test]
    fn apply_patch_rejects_value_of_wrong_kind() {
        let mut save = settings_save(vec![("ExpRate", crate::props::float_property(1.0))]);
        let error = apply_patch(&mut save, &[patch("ExpRate", serde_json::json!("nope"))]).unwrap_err();
        assert!(format!("{error}").contains("ExpRate"));
    }

    #[test]
    fn apply_patch_rejects_bare_enum_variant() {
        let mut save = settings_save(vec![]);
        // Enum values must be fully qualified in both directions.
        let error = apply_patch(&mut save, &[patch("Difficulty", serde_json::json!("Custom"))]).unwrap_err();
        assert!(format!("{error}").contains("Difficulty"));
    }

    #[test]
    fn apply_patch_accepts_fully_qualified_enum_variant() {
        let mut save = settings_save(vec![]);
        let dirty = apply_patch(
            &mut save,
            &[patch("Difficulty", serde_json::json!("EPalOptionWorldDifficulty::Custom"))],
        )
        .unwrap();
        assert!(dirty);
        assert_eq!(
            read_settings(&save)[0].value,
            serde_json::json!("EPalOptionWorldDifficulty::Custom")
        );
    }

    #[test]
    fn apply_patch_round_trips_arrays() {
        let mut save = settings_save(vec![]);
        apply_patch(
            &mut save,
            &[
                patch("CrossplayPlatforms", serde_json::json!(["EPalAllowConnectPlatform::Steam"])),
                patch("DenyTechnologyList", serde_json::json!(["AIcore", "Accessory_AirDash1"])),
            ],
        )
        .unwrap();

        let entries = read_settings(&save);
        assert_eq!(entries[0].value, serde_json::json!(["EPalAllowConnectPlatform::Steam"]));
        assert_eq!(entries[1].value, serde_json::json!(["AIcore", "Accessory_AirDash1"]));
    }

    #[test]
    fn apply_patch_rejects_mixed_patch_without_mutating_anything() {
        // Test with VALID entry first, then INVALID entry
        let mut save = settings_save(vec![("ExpRate", crate::props::float_property(1.0))]);
        let error = apply_patch(
            &mut save,
            &[
                patch("ExpRate", serde_json::json!(9.0)),
                patch("NoSuchSetting", serde_json::json!(1)),
            ],
        )
        .unwrap_err();
        assert!(format!("{error}").contains("NoSuchSetting"));
        // Verify the valid entry was NOT applied because we decode-then-mutate
        let entries = read_settings(&save);
        assert_eq!(entries[0].value, serde_json::json!(1.0), "ExpRate must not be mutated when patch is rejected");

        // Test with INVALID entry first, then VALID entry
        let mut save = settings_save(vec![("ExpRate", crate::props::float_property(1.0))]);
        let error = apply_patch(
            &mut save,
            &[
                patch("NoSuchSetting", serde_json::json!(1)),
                patch("ExpRate", serde_json::json!(9.0)),
            ],
        )
        .unwrap_err();
        assert!(format!("{error}").contains("NoSuchSetting"));
        // Verify the valid entry was NOT applied even though it comes second
        let entries = read_settings(&save);
        assert_eq!(entries[0].value, serde_json::json!(1.0), "ExpRate must not be mutated when patch is rejected");
    }

    #[test]
    fn apply_patch_writing_an_identical_enum_value_is_not_dirty() {
        let mut save = settings_save(vec![(
            "Difficulty",
            crate::props::enum_property("EPalOptionWorldDifficulty::Custom"),
        )]);
        // A no-op edit on an enum must not trigger a rewrite of the user's file.
        assert!(!apply_patch(
            &mut save,
            &[patch("Difficulty", serde_json::json!("EPalOptionWorldDifficulty::Custom"))]
        )
        .unwrap());
    }
}

#[cfg(test)]
mod world_option_corpus;
