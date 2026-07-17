//! `WorldOption.sav` settings access.
//!
//! `WORLD_OPTION_SETTINGS` is the single source of truth for every setting's GVAS
//! type and its write schema. The table was generated from the real testdata corpus
//! (7 files, zero tag conflicts) rather than hand-written -- see the spec's Risks
//! section. `world_option_table_matches_corpus` in `tests/world_option_corpus.rs`
//! is what keeps it honest when Palworld ships new settings.
//!
//! The file is SPARSE: real saves carry anywhere from 87 to 119 of these keys, and
//! the `Version` property (always 101) does not discriminate. Presence is read from
//! the data, never inferred.

use uesave::{PropertyTagDataPartial, PropertyTagPartial, PropertyType};

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
pub fn ensure_world_option_schemas(save: &mut uesave::Save) {
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
            uesave::PropertyTagDataPartial::Array(inner) => match *inner {
                uesave::PropertyTagDataPartial::Enum(name, second) => {
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
            uesave::PropertyTagDataPartial::Array(inner) => assert!(matches!(
                *inner,
                uesave::PropertyTagDataPartial::Other(uesave::PropertyType::NameProperty)
            )),
            other => panic!("expected Array, got {other:?}"),
        }
    }

    #[test]
    fn tag_for_enum_carries_name_and_none_second_field() {
        let tag = tag_for(WoKind::Enum("EPalOptionWorldDifficulty"));
        match tag.data {
            uesave::PropertyTagDataPartial::Enum(name, second) => {
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
}
