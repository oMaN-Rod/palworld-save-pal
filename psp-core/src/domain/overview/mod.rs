//! Whole-save overview statistics, ported from PalSavTools' Python overview
//! services to pure Rust.
//!
//! Public entry point: [`overview_stats`] — the single-pass aggregation behind
//! the Overview dashboard (totals, traits, conditions, composition, top
//! species, the player leaderboard, and the "pals needing review" anomaly
//! report). The pieces it orchestrates live in focused submodules:
//!
//! * [`illegal_pals`] — the pal legality validator ("illegal pals checker"),
//!   including the game-accurate MaxHP ceiling formula.
//! * [`leaderboard`] — the player leaderboard and its ranking metrics.
//! * [`power`] — the per-pal raw-power score behind the power ranking.
//! * [`dps`] — the Dimensional Pal Storage slot walker extending the legality
//!   scan past `Level.sav`.
//! * [`catalogs`] — the game-data catalogs everything resolves against.
//! * [`classify`] — boss-prefix/gender/sick/fainted classification helpers.
//! * [`composition`] — level/gender/talent/skill composition accumulation.
//! * [`anomalies`] — the flagged-pal report collection.
//!
//! Everything is computed live from the parsed `Level.sav` tree (plus the
//! per-player DPS saves for the legality scan) against the bundled game data
//! — no precomputed or hard-coded values.

mod anomalies;
mod catalogs;
mod classify;
mod composition;
mod dps;
mod illegal_pals;
mod leaderboard;
mod power;

pub use illegal_pals::{
    ILLEGAL_ACTIVE, ILLEGAL_COMBINATION, ILLEGAL_HP, ILLEGAL_LEVEL, ILLEGAL_PASSIVE, ILLEGAL_RANK,
    ILLEGAL_SPECIES, SAFE_ACTIVE_SLOTS, SAFE_IV_MAX, SAFE_LEVEL_MAX, SAFE_PASSIVE_SLOTS,
    SAFE_RANK_MAX, SAFE_SOUL_MAX, SUSPICIOUS_ACTIVE_SLOTS, SUSPICIOUS_DUPLICATE_ACTIVE,
    SUSPICIOUS_DUPLICATE_PASSIVE, SUSPICIOUS_PASSIVE_SLOTS, SUSPICIOUS_SOUL_RANK,
    SUSPICIOUS_TALENT, severity_of,
};

use std::collections::HashMap;

use crate::dto::overview::{
    OverviewCondition, OverviewSpeciesCount, OverviewStats, OverviewTotals, OverviewTraits,
};
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::props;
use crate::session::SaveSession;
use crate::ue::Properties;

use crate::domain::pal::param;
use crate::domain::{guild_tail, summaries, world};

use anomalies::{AnomalyCollector, SOURCE_DPS, SOURCE_WORLD};
use catalogs::OverviewCatalogs;
use classify::{canonical_character_key, is_boss_id, is_fainted, is_sick, strip_boss_prefix};
use composition::{CompositionAccumulator, OrderedCounter};
use leaderboard::{OwnerMetrics, top_players};

/// How many species the overview's "top species" card previews.
const TOP_SPECIES_SIZE: usize = 6;

const GROUP_TYPE_GUILD: &str = "EPalGroupType::Guild";

/// Computes the full Overview dataset in one pass over
/// `CharacterSaveParameterMap`, plus small reads of the group / base-camp /
/// item-container maps for the guild, base, and container counts, a legality
/// scan of every player's DPS (Dimensional Pal Storage) save, and a pure
/// ranking over the eager player summaries for the leaderboard.
pub fn overview_stats(
    session: &SaveSession,
    game_data: &GameData,
) -> Result<OverviewStats, CoreError> {
    let catalogs = OverviewCatalogs::from_game_data(game_data);
    let character_entries = session.character_map()?;
    let group_entries = session.group_map()?;
    let bases = session.base_camp_map().map_or(0, |entries| entries.len()) as i64;
    let containers = session.item_container_map()?.len() as i64;
    let guilds = group_entries
        .iter()
        .filter(|entry| {
            props::struct_properties(&entry.value).and_then(|value_properties| {
                props::get(value_properties, &["GroupType"]).and_then(props::as_enum)
            }) == Some(GROUP_TYPE_GUILD)
        })
        .count() as i64;

    let mut totals = OverviewTotals {
        guilds,
        bases,
        containers,
        ..OverviewTotals::default()
    };
    let mut traits = OverviewTraits::default();
    let mut condition = OverviewCondition::default();

    let mut species_counter = OrderedCounter::new();
    // First-seen original-case form per species key, for display fallback.
    let mut species_display: HashMap<String, String> = HashMap::new();
    let mut composition = CompositionAccumulator::new();
    let mut anomalies = AnomalyCollector::new();
    // Leaderboard inputs, collected in the same pass: per-owner ranking
    // metrics and player levels straight off the character map, so players
    // whose own save files are missing from the world still rank (reference
    // behavior).
    let mut owner_metrics: HashMap<uuid::Uuid, OwnerMetrics> = HashMap::new();
    let mut player_levels: HashMap<uuid::Uuid, i64> = HashMap::new();

    for entry in character_entries {
        let Some(save_parameter) = world::entry_save_parameter(entry) else {
            continue;
        };
        if summaries::is_player_entry(save_parameter) {
            totals.players += 1;
            if let Some(level) = param(save_parameter, "Level").and_then(props::as_byte_number) {
                if let Some(uid) = world::entry_player_uid(entry) {
                    player_levels.entry(uid).or_insert(level as i64);
                }
            }
            continue;
        }

        // Composition buckets every non-player entry with a readable
        // SaveParameter, even one too corrupt to classify below.
        composition.add(save_parameter);

        // The ownership tally counts every non-player entry with an owner —
        // including ones too corrupt to classify — like the reference
        // implementation's precomputed pal counts.
        let owner = param(save_parameter, "OwnerPlayerUId").and_then(props::as_uuid);
        if let Some(owner) = owner {
            owner_metrics.entry(owner).or_default().pal_count += 1;
        }

        let character_id = param(save_parameter, "CharacterID")
            .and_then(props::as_str)
            .unwrap_or("");
        if character_id.is_empty() {
            continue;
        }

        let stripped = strip_boss_prefix(character_id);
        let species_key = stripped.to_lowercase();
        if catalogs.is_human_npc(character_id) {
            totals.human_npcs += 1;
        } else {
            totals.creature_pals += 1;
        }
        species_counter.add(species_key.clone());
        species_display
            .entry(species_key)
            .or_insert_with(|| stripped.to_string());

        if is_boss_id(character_id) {
            traits.boss_pals += 1;
        }
        let is_lucky = param(save_parameter, "IsRarePal")
            .and_then(props::as_bool)
            .unwrap_or(false);
        if is_lucky {
            traits.rare_pals += 1;
        }
        if param(save_parameter, "bIsAwakening")
            .and_then(props::as_bool)
            .unwrap_or(false)
        {
            traits.awakened_pals += 1;
        }
        if is_sick(save_parameter) {
            condition.sick_pals += 1;
        }
        if is_fainted(save_parameter) {
            condition.fainted_pals += 1;
        }

        let level = current_level(save_parameter);
        if let Some(owner) = owner {
            owner_metrics.entry(owner).or_default().note_pal(
                level,
                is_lucky,
                power::pal_power_score(save_parameter, character_id, &catalogs),
            );
        }

        let codes = illegal_pals::detect_pal_issues(save_parameter, character_id, &catalogs);
        if !codes.is_empty() {
            anomalies.record(anomalies::FlaggedPal {
                instance_id: world::entry_instance_id(entry).unwrap_or(uuid::Uuid::nil()),
                owner_uid: owner,
                source: SOURCE_WORLD,
                character_id,
                character_key: canonical_character_key(character_id, game_data),
                level,
                codes,
            });
        }
    }

    // Pals parked in a player's Dimensional Pal Storage live only in the
    // per-player `_dps.sav` files — the character map above never sees them.
    // Scan each one with the same validator and credit the storing player.
    let mut dps_pal_counts: HashMap<uuid::Uuid, i64> = HashMap::new();
    for (player_uid, file_ref) in &session.player_file_refs {
        let Ok(Some(dps_bytes)) = file_ref.dps_bytes() else {
            continue;
        };
        let Ok(dps_save) = crate::session::parse_palworld_save(&dps_bytes) else {
            continue;
        };
        for pal in dps::pals_in(&dps_save) {
            *dps_pal_counts.entry(*player_uid).or_insert(0) += 1;
            let codes =
                illegal_pals::detect_pal_issues(pal.save_parameter, pal.character_id, &catalogs);
            if !codes.is_empty() {
                anomalies.record(anomalies::FlaggedPal {
                    instance_id: pal.instance_id,
                    owner_uid: Some(*player_uid),
                    source: SOURCE_DPS,
                    character_id: pal.character_id,
                    character_key: canonical_character_key(pal.character_id, game_data),
                    level: current_level(pal.save_parameter),
                    codes,
                });
            }
        }
    }

    totals.pals = totals.creature_pals + totals.human_npcs;
    totals.species = species_counter.len() as i64;

    // Guild rosters supply every member's name and membership even when the
    // player's own save file never made it into the world (shared/migrated
    // multiplayer saves), matching the reference's roster-joined preview.
    let mut roster: Vec<(uuid::Uuid, String)> = Vec::new();
    for entry in group_entries {
        if guild_tail::entry_group_type(entry).as_deref() != Some(GROUP_TYPE_GUILD) {
            continue;
        }
        let Some(group_data) = guild_tail::entry_group_data(entry) else {
            continue;
        };
        let Some(guild) = guild_tail::as_guild(group_data) else {
            continue;
        };
        roster.extend(guild_tail::guild_roster(guild));
    }

    let top_species = species_counter
        .top(TOP_SPECIES_SIZE)
        .into_iter()
        .map(|species| {
            let key = game_data
                .pal_lookup()
                .lower_to_canonical
                .get(&species.skill)
                .cloned()
                .unwrap_or_else(|| {
                    species_display
                        .get(&species.skill)
                        .cloned()
                        .unwrap_or_else(|| species.skill.clone())
                });
            OverviewSpeciesCount {
                key,
                count: species.count,
            }
        })
        .collect();

    Ok(OverviewStats {
        totals,
        traits,
        condition,
        composition: composition.finish(),
        top_species,
        top_players: top_players(
            session,
            &roster,
            &owner_metrics,
            &player_levels,
            &dps_pal_counts,
        ),
        anomalies: anomalies.finish(),
    })
}

/// The level shown on anomaly rows; absent reads as 1, like the validator.
fn current_level(save_parameter: &Properties) -> i64 {
    param(save_parameter, "Level")
        .and_then(props::as_byte_number)
        .unwrap_or(1) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::summary::PlayerSummary;
    use crate::ue::{Byte, MapEntry, Property, StructValue, ValueVec};

    const PLAYER_ONE: &str = "11111111-1111-1111-1111-111111111111";

    fn game_data() -> GameData {
        GameData::from_entries([
            (
                "pals".to_string(),
                r#"{
                    "Alpaca": {"is_pal": true, "scaling": {"hp": 90, "attack": 75, "defense": 90}, "friendship_hp": 4.5},
                    "Anubis": {"is_pal": true, "scaling": {"hp": 120, "attack": 130, "defense": 120}, "friendship_hp": 4.5},
                    "Human": {"is_pal": false, "scaling": {"hp": 70, "attack": 70, "defense": 70}, "friendship_hp": 1.0},
                    "Sheepball": {"is_pal": true, "scaling": {"hp": 80, "attack": 80, "defense": 80}, "friendship_hp": 4.5}
                }"#.to_string(),
            ),
            (
                "passive_skills".to_string(),
                r#"{"Legend": {"effects": [{"type": "MaxHP", "value": 20.0, "target": "ToSelf"}]}}"#.to_string(),
            ),
            (
                "active_skills".to_string(),
                r#"{"EPalWazaID::AirCanon": {"power": 40}}"#.to_string(),
            ),
            (
                "friendship".to_string(),
                r#"{
                    "Friendship_Rank_0": {"rank": 0, "required_point": 0},
                    "Friendship_Rank_1": {"rank": 1, "required_point": 6000},
                    "Friendship_Rank_2": {"rank": 2, "required_point": 13000}
                }"#.to_string(),
            ),
        ])
        .unwrap()
    }

    fn str_property(value: &str) -> Property {
        Property::Str(value.to_string())
    }

    fn byte_property(value: u8) -> Property {
        Property::Byte(Byte::Byte(value))
    }

    fn guid_property(text: &str) -> Property {
        Property::Struct(StructValue::Guid(
            serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap(),
        ))
    }

    fn character_entry(character_id: &str, save_parameter: Properties) -> MapEntry {
        let mut parameters = save_parameter;
        parameters.insert("CharacterID", str_property(character_id));
        let mut object = Properties::default();
        object.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(parameters)),
        );
        let character_data = crate::ue::games::palworld::PalCharacterData {
            object,
            unknown_bytes: [0; 4],
            group_id: crate::ue::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterData(
                character_data,
            ))),
        );
        let mut key_properties = Properties::default();
        key_properties.insert(
            "PlayerUId",
            guid_property("00000000-0000-0000-0000-000000000000"),
        );
        key_properties.insert(
            "InstanceId",
            guid_property("aaaaaaaa-0000-0000-0000-000000000001"),
        );
        MapEntry {
            key: Property::Struct(StructValue::Struct(key_properties)),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    fn guild_entry(guild_id: &str) -> MapEntry {
        let mut value_properties = Properties::default();
        value_properties.insert("GroupType", Property::Enum(GROUP_TYPE_GUILD.to_string()));
        MapEntry {
            key: guid_property(guild_id),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    fn minimal_session(characters: Vec<MapEntry>, groups: Vec<MapEntry>) -> SaveSession {
        let mut world_save_data = Properties::default();
        world_save_data.insert("CharacterSaveParameterMap", Property::Map(characters));
        world_save_data.insert("GroupSaveDataMap", Property::Map(groups));
        world_save_data.insert(
            "BaseCampSaveData",
            Property::Map(vec![MapEntry {
                key: guid_property("44444444-4444-4444-4444-444444444444"),
                value: Property::Struct(StructValue::Struct(Properties::default())),
            }]),
        );
        world_save_data.insert(
            "ItemContainerSaveData",
            Property::Map(vec![
                MapEntry {
                    key: guid_property("55555555-5555-5555-5555-555555555555"),
                    value: Property::Struct(StructValue::Struct(Properties::default())),
                },
                MapEntry {
                    key: guid_property("66666666-6666-6666-6666-666666666666"),
                    value: Property::Struct(StructValue::Struct(Properties::default())),
                },
            ]),
        );
        let mut root_properties = Properties::default();
        root_properties.insert(
            "worldSaveData",
            Property::Struct(StructValue::Struct(world_save_data)),
        );
        let level = crate::ue::Save {
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
                properties: root_properties,
            },
            extra: Vec::new(),
        };
        SaveSession::new_for_tests(crate::session::SaveKind::InMemory, level)
    }

    /// End-to-end aggregation over a synthetic world exercising every bucket.
    #[test]
    fn overview_stats_aggregates_every_section() {
        let game_data = game_data();
        let mut session = minimal_session(
            vec![
                {
                    // Player entry: counted as player, excluded everywhere else.
                    let mut save_parameter = Properties::default();
                    save_parameter.insert("IsPlayer", Property::Bool(true));
                    save_parameter.insert("NickName", str_property("Tester"));
                    save_parameter.insert("Level", byte_property(9));
                    character_entry("Tester", save_parameter)
                },
                {
                    // Clean creature pal, level 30 male with a passive+active,
                    // owned by the player below.
                    let mut save_parameter = Properties::default();
                    save_parameter.insert("Level", byte_property(30));
                    save_parameter.insert("Gender", Property::Enum("EPalGenderType::Male".into()));
                    save_parameter.insert("Talent_HP", byte_property(10));
                    save_parameter.insert("OwnerPlayerUId", guid_property(PLAYER_ONE));
                    save_parameter.insert(
                        "PassiveSkillList",
                        Property::Array(ValueVec::Name(vec!["Legend".to_string()])),
                    );
                    save_parameter.insert(
                        "EquipWaza",
                        Property::Array(ValueVec::Enum(vec!["EPalWazaID::AirCanon".to_string()])),
                    );
                    character_entry("Alpaca", save_parameter)
                },
                {
                    // Sick + fainted boss pal, level 50 female.
                    let mut save_parameter = Properties::default();
                    save_parameter.insert("Level", byte_property(50));
                    save_parameter
                        .insert("Gender", Property::Enum("EPalGenderType::Female".into()));
                    save_parameter.insert(
                        "WorkerSick",
                        Property::Enum("EPalStatusSickType::Cold".into()),
                    );
                    save_parameter.insert("PalReviveTimer", Property::Int(30));
                    character_entry("BOSS_Alpaca", save_parameter)
                },
                {
                    // Human NPC, no gender → unknown bucket, level defaults to 1.
                    character_entry("Human", Properties::default())
                },
                {
                    // Hacked pal: impossible level + over-max talent (level 200
                    // also lands in the top bracket).
                    let mut save_parameter = Properties::default();
                    save_parameter.insert("Level", byte_property(200));
                    save_parameter.insert("Talent_HP", byte_property(255));
                    character_entry("Sheepball", save_parameter)
                },
            ],
            vec![guild_entry("33333333-3333-3333-3333-333333333333")],
        );

        let player_one: uuid::Uuid = PLAYER_ONE.parse().unwrap();
        session.player_summary_order = vec![player_one];
        session.player_summaries.insert(
            player_one,
            PlayerSummary {
                uid: player_one,
                nickname: "Tester".to_string(),
                level: Some(9),
                guild_id: None,
                pal_count: 4,
                last_online_time: None,
                loaded: false,
            },
        );

        let stats = overview_stats(&session, &game_data).unwrap();

        assert_eq!(stats.totals.players, 1);
        assert_eq!(stats.totals.creature_pals, 3);
        assert_eq!(stats.totals.human_npcs, 1);
        assert_eq!(stats.totals.pals, 4);
        // Sheepball, Alpaca, Human — BOSS_Alpaca folds into alpaca.
        assert_eq!(stats.totals.species, 3);
        assert_eq!(stats.totals.guilds, 1);
        assert_eq!(stats.totals.bases, 1);
        assert_eq!(stats.totals.containers, 2);

        assert_eq!(stats.traits.boss_pals, 1);
        assert_eq!(stats.traits.rare_pals, 0);
        assert_eq!(stats.traits.awakened_pals, 0);
        assert_eq!(stats.condition.sick_pals, 1);
        assert_eq!(stats.condition.fainted_pals, 1);

        // Composition covers all four non-player entries.
        assert_eq!(stats.composition.gender.male, 1);
        assert_eq!(stats.composition.gender.female, 1);
        assert_eq!(stats.composition.gender.unknown, 2);
        assert_eq!(
            stats
                .composition
                .level_brackets
                .iter()
                .map(|bracket| (bracket.label, bracket.count))
                .collect::<Vec<_>>(),
            vec![("1-20", 1), ("21-40", 1), ("41-60", 1), ("61-80", 1)]
        );
        // (30 + 50 + 1 + 200) / 4 = 70.25 → 70.2 under banker's rounding.
        assert_eq!(stats.composition.avg_level, 70.2);
        // (10 + 0 + 0 + 255) / 4 = 66.25 → 66.2 likewise.
        assert_eq!(stats.composition.talent_avg.hp, 66.2);
        assert_eq!(stats.composition.top_passives.len(), 1);
        assert_eq!(stats.composition.top_passives[0].skill, "Legend");
        assert_eq!(
            stats.composition.top_actives[0].skill,
            "EPalWazaID::AirCanon"
        );

        assert_eq!(stats.top_species.len(), 3);
        assert_eq!(stats.top_species[0].key, "Alpaca");
        assert_eq!(stats.top_species[0].count, 2);

        assert_eq!(stats.top_players.len(), 1);
        assert_eq!(stats.top_players[0].nickname, "Tester");
        // The character-map tally (one owned pal) wins over the summary's
        // stale count of 4, and the full metric bundle travels along.
        assert_eq!(stats.top_players[0].pal_count, 1);
        assert_eq!(stats.top_players[0].lucky_count, 0);
        assert_eq!(stats.top_players[0].avg_pal_level, Some(30.0));
        assert_eq!(stats.top_players[0].max_pal_level, Some(30));
        assert!(stats.top_players[0].total_power > 0);
        assert_eq!(stats.top_players[0].dps_pal_count, 0);

        assert_eq!(stats.anomalies.pal_count, 1);
        assert_eq!(stats.anomalies.danger_count, 1);
        assert_eq!(
            stats
                .anomalies
                .by_code
                .iter()
                .map(|entry| (entry.code, entry.count))
                .collect::<Vec<_>>(),
            vec![(ILLEGAL_LEVEL, 1), (SUSPICIOUS_TALENT, 1)]
        );
        assert_eq!(stats.anomalies.flagged[0].severity, "danger");
        assert_eq!(stats.anomalies.flagged[0].character_key, "Sheepball");
        assert_eq!(stats.anomalies.flagged[0].source, "world");
        assert_eq!(stats.anomalies.flagged[0].owner_uid, None);
    }

    /// A world with no maps at all still aggregates to zeroed sections.
    #[test]
    fn overview_stats_survives_an_empty_world() {
        let session = minimal_session(vec![], vec![]);
        let stats = overview_stats(&session, &game_data()).unwrap();
        assert_eq!(stats.totals.players, 0);
        assert_eq!(stats.totals.pals, 0);
        assert_eq!(stats.totals.species, 0);
        assert_eq!(stats.totals.guilds, 0);
        assert_eq!(stats.top_species.len(), 0);
        assert_eq!(stats.top_players.len(), 0);
        assert_eq!(stats.anomalies.pal_count, 0);
        assert_eq!(stats.composition.avg_level, 0.0);
    }

    fn dps_slot(character_id: &str, save_parameter: Properties) -> crate::ue::StructValue {
        let mut parameters = save_parameter;
        parameters.insert("CharacterID", str_property(character_id));
        let mut slot_props = Properties::default();
        slot_props.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(parameters)),
        );
        let mut inner = Properties::default();
        inner.insert(
            "InstanceId",
            guid_property("bbbbbbbb-0000-0000-0000-000000000001"),
        );
        slot_props.insert("InstanceId", Property::Struct(StructValue::Struct(inner)));
        StructValue::Struct(slot_props)
    }

    /// A hand-built DPS save: two slots run through the walker and the
    /// validator the byte-level scan loop uses, without needing serializable
    /// fixture bytes.
    #[test]
    fn overview_stats_flags_dps_pals_through_the_walker() {
        let game_data = game_data();

        let mut hacked = Properties::default();
        hacked.insert("Level", byte_property(200));
        hacked.insert("Talent_HP", byte_property(255));
        let mut clean = Properties::default();
        clean.insert("Level", byte_property(12));
        let slots = vec![dps_slot("Sheepball", hacked), dps_slot("Alpaca", clean)];
        let mut root_properties = Properties::default();
        root_properties.insert(
            "SaveParameterArray",
            Property::Array(ValueVec::Struct(slots)),
        );
        let dps_save = crate::ue::Save {
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
                properties: root_properties,
            },
            extra: Vec::new(),
        };

        let pals = dps::pals_in(&dps_save);
        assert_eq!(pals.len(), 2);
        let catalogs = OverviewCatalogs::from_game_data(&game_data);
        assert_eq!(
            illegal_pals::detect_pal_issues(
                pals[0].save_parameter,
                pals[0].character_id,
                &catalogs
            ),
            vec![ILLEGAL_LEVEL, SUSPICIOUS_TALENT]
        );
        assert!(
            illegal_pals::detect_pal_issues(
                pals[1].save_parameter,
                pals[1].character_id,
                &catalogs
            )
            .is_empty()
        );
    }

    /// Loads a committed fixture save (with its per-player `_dps.sav` files)
    /// from `tests/fixtures/saves/<name>/`, mirroring the player-domain
    /// fixture loader.
    fn load_fixture_session(name: &str) -> SaveSession {
        let save_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves")
            .join(name);
        let level_sav_bytes = std::fs::read(save_dir.join("Level.sav")).unwrap();

        let mut player_file_refs: std::collections::BTreeMap<
            uuid::Uuid,
            crate::session::PlayerFileData,
        > = std::collections::BTreeMap::new();
        for entry in std::fs::read_dir(save_dir.join("Players"))
            .unwrap()
            .flatten()
        {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "sav") {
                continue;
            }
            let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
            let uid: uuid::Uuid = stem.replace("_dps", "").parse().unwrap();
            let file_ref =
                player_file_refs
                    .entry(uid)
                    .or_insert(crate::session::PlayerFileData::Paths {
                        sav: None,
                        dps: None,
                    });
            if let crate::session::PlayerFileData::Paths { sav, dps } = file_ref {
                if stem.ends_with("_dps") {
                    *dps = Some(path);
                } else {
                    *sav = Some(path);
                }
            }
        }

        SaveSession::load(
            crate::session::SaveKind::Steam {
                level_path: save_dir.join("Level.sav"),
            },
            save_dir.to_string_lossy().into_owned(),
            "steam",
            &level_sav_bytes,
            None,
            None,
            player_file_refs,
            None,
            true,
            &crate::progress::null_progress(),
        )
        .expect("load fixture session")
    }

    fn repo_game_data() -> GameData {
        let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
        GameData::load(&json_dir).expect("data dir")
    }

    /// End-to-end on the committed `v1_relics` fixture (whose player
    /// `B38A3AB1…` stores 9 pals in their `_dps.sav`): DPS pals count on the
    /// leaderboard, the scan reaches them, and every row's new metrics are
    /// well-formed on real data.
    #[test]
    fn overview_stats_scans_dps_pals_on_a_real_save() {
        let session = load_fixture_session("v1_relics");
        // The fixture's one player with a `_dps.sav` (Steam PUID B38A3AB1…).
        let dps_owner = session
            .player_file_refs
            .iter()
            .find(|(_, file_ref)| matches!(file_ref.dps_bytes(), Ok(Some(_))))
            .map(|(uid, _)| *uid)
            .expect("fixture has a dps save");
        let stats = overview_stats(&session, &repo_game_data()).unwrap();

        let row = stats
            .top_players
            .iter()
            .find(|row| row.uid == dps_owner)
            .expect("dps owner ranks");
        assert_eq!(row.dps_pal_count, 9, "fixture dps save stores 9 pals");

        for flagged in &stats.anomalies.flagged {
            assert!(flagged.source == "world" || flagged.source == "dps");
            if flagged.source == "dps" {
                assert!(flagged.owner_uid.is_some());
            }
        }
        for row in &stats.top_players {
            assert!(row.pal_count >= 0);
            assert!(row.lucky_count >= 0);
            assert!(row.total_power >= 0);
            if let Some(avg) = row.avg_pal_level {
                assert!((1.0..=255.0).contains(&avg));
            }
        }
    }
}
