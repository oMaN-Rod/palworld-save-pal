//! Overview wire DTOs: the whole-save statistics dashboard served by
//! `get_overview_stats`. Field declaration order is a wire contract: `serde`
//! serializes in declaration order and the frontend consumes this JSON as-is
//! over the WebSocket.
//!
//! Species and skill rows carry raw catalog keys (`character_key`, passive
//! asset id, `EPalWazaID::…`), never display names — the frontend resolves
//! localized names and icons from its own game-data stores, so the overview
//! follows the app's language for free.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct OverviewTotals {
    pub players: i64,
    pub pals: i64,
    pub creature_pals: i64,
    pub human_npcs: i64,
    pub species: i64,
    pub guilds: i64,
    pub bases: i64,
    pub containers: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct OverviewTraits {
    pub boss_pals: i64,
    pub rare_pals: i64,
    pub awakened_pals: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct OverviewCondition {
    pub sick_pals: i64,
    pub fainted_pals: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct OverviewGenderSplit {
    pub male: i64,
    pub female: i64,
    pub unknown: i64,
}

/// A level-range bucket. The label travels on the wire because the ranges are
/// numeric, not localizable text; the top bucket reads "61-80" but is
/// everything above 60 (matching the reference implementation's four buckets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverviewLevelBracket {
    pub label: &'static str,
    pub count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Default)]
pub struct OverviewTalentAvg {
    pub hp: f64,
    pub attack: f64,
    pub defense: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverviewSkillCount {
    /// Raw catalog id: passive asset (`HP_ACC_up1`) or `EPalWazaID::…` active.
    pub skill: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct OverviewComposition {
    pub avg_level: f64,
    pub gender: OverviewGenderSplit,
    pub level_brackets: Vec<OverviewLevelBracket>,
    pub talent_avg: OverviewTalentAvg,
    pub top_passives: Vec<OverviewSkillCount>,
    pub top_actives: Vec<OverviewSkillCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverviewSpeciesCount {
    /// Canonical `pals.json` key when known, else the boss-stripped raw id.
    pub key: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverviewPlayerRow {
    pub uid: uuid::Uuid,
    pub nickname: String,
    pub level: Option<i64>,
    pub pal_count: i64,
}

/// One flagged pal from the legality scan. `codes` are the stable machine
/// codes (`ILLEGAL_HP`, `SUSPICIOUS_TALENT`, …) the frontend translates to
/// localized text; `severity` is `"danger"` when any code is a danger code,
/// else `"warning"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverviewAnomalyRow {
    pub instance_id: uuid::Uuid,
    pub character_id: String,
    /// Canonical `pals.json` key when known (for the icon), else stripped id.
    pub character_key: String,
    pub level: i64,
    pub severity: &'static str,
    pub codes: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverviewCodeCount {
    pub code: &'static str,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct OverviewAnomalies {
    pub pal_count: i64,
    pub danger_count: i64,
    /// Full per-code tallies, sorted by count descending (first-seen order on
    /// ties) — the UI shows the breakdown even when `flagged` is truncated.
    pub by_code: Vec<OverviewCodeCount>,
    /// Every flagged pal, in save-file order. The dashboard previews the
    /// first 25 and expands on demand.
    pub flagged: Vec<OverviewAnomalyRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OverviewStats {
    pub totals: OverviewTotals,
    pub traits: OverviewTraits,
    pub condition: OverviewCondition,
    pub composition: OverviewComposition,
    pub top_species: Vec<OverviewSpeciesCount>,
    pub top_players: Vec<OverviewPlayerRow>,
    pub anomalies: OverviewAnomalies,
}
