//! "Pals needing review" collection: every pal the legality validator flags,
//! with per-code tallies and danger/warning severities. Mirrors the reference
//! project's anomalies block (`pal_count`, `danger_count`, `by_code`,
//! preview rows) — except the full flagged list travels on the wire and the
//! dashboard previews it client-side.

use std::collections::HashMap;

use crate::dto::overview::{OverviewAnomalies, OverviewAnomalyRow, OverviewCodeCount};

use super::illegal_pals::severity_of;

/// `source` value for pals flagged out of `Level.sav`'s character map.
pub(crate) const SOURCE_WORLD: &str = "world";
/// `source` value for pals flagged out of a player's DPS (Dimensional Pal
/// Storage) save.
pub(crate) const SOURCE_DPS: &str = "dps";

pub(crate) struct AnomalyCollector {
    by_code: HashMap<&'static str, i64>,
    code_order: Vec<&'static str>,
    flagged: Vec<OverviewAnomalyRow>,
    danger_count: i64,
}

/// One flagged pal handed to [`AnomalyCollector::record`].
pub(crate) struct FlaggedPal<'a> {
    pub(crate) instance_id: uuid::Uuid,
    /// The owning player, when known (always set for DPS rows).
    pub(crate) owner_uid: Option<uuid::Uuid>,
    /// [`SOURCE_WORLD`] or [`SOURCE_DPS`].
    pub(crate) source: &'static str,
    pub(crate) character_id: &'a str,
    pub(crate) character_key: String,
    pub(crate) level: i64,
    /// The validator's non-empty issue list.
    pub(crate) codes: Vec<&'static str>,
}

impl AnomalyCollector {
    pub(crate) fn new() -> Self {
        AnomalyCollector {
            by_code: HashMap::new(),
            code_order: Vec::new(),
            flagged: Vec::new(),
            danger_count: 0,
        }
    }

    /// Records one flagged pal. Severity is `"danger"` when any code is a
    /// danger code, else `"warning"`.
    pub(crate) fn record(&mut self, pal: FlaggedPal<'_>) {
        let FlaggedPal {
            instance_id,
            owner_uid,
            source,
            character_id,
            character_key,
            level,
            codes,
        } = pal;
        let is_danger = codes.iter().any(|code| severity_of(code) == "danger");
        if is_danger {
            self.danger_count += 1;
        }
        for code in &codes {
            if let Some(count) = self.by_code.get_mut(code) {
                *count += 1;
            } else {
                self.code_order.push(code);
                self.by_code.insert(code, 1);
            }
        }
        self.flagged.push(OverviewAnomalyRow {
            instance_id,
            character_id: character_id.to_string(),
            character_key,
            level,
            severity: if is_danger { "danger" } else { "warning" },
            codes,
            owner_uid,
            source,
        });
    }

    /// Per-code tallies sorted by count descending, first-seen order on ties.
    pub(crate) fn finish(self) -> OverviewAnomalies {
        let mut ranked_codes = self.code_order;
        ranked_codes.sort_by(|a, b| self.by_code[b].cmp(&self.by_code[a]));
        OverviewAnomalies {
            pal_count: self.flagged.len() as i64,
            danger_count: self.danger_count,
            by_code: ranked_codes
                .into_iter()
                .map(|code| OverviewCodeCount {
                    code,
                    count: self.by_code[code],
                })
                .collect(),
            flagged: self.flagged,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::illegal_pals::{ILLEGAL_LEVEL, SUSPICIOUS_TALENT};
    use super::*;

    fn uid(text: &str) -> uuid::Uuid {
        text.parse().unwrap()
    }

    #[test]
    fn tallies_codes_ranks_by_count_and_marks_severity() {
        let mut collector = AnomalyCollector::new();
        // First pal: one warning code, from a player's DPS save.
        collector.record(FlaggedPal {
            instance_id: uid("aaaaaaaa-0000-0000-0000-000000000001"),
            owner_uid: Some(uid("11111111-1111-1111-1111-111111111111")),
            source: SOURCE_DPS,
            character_id: "Sheepball",
            character_key: "Sheepball".to_string(),
            level: 50,
            codes: vec![SUSPICIOUS_TALENT],
        });
        // Second pal: a danger + a warning code → danger severity, from the world.
        collector.record(FlaggedPal {
            instance_id: uid("aaaaaaaa-0000-0000-0000-000000000002"),
            owner_uid: None,
            source: SOURCE_WORLD,
            character_id: "Sheepball",
            character_key: "Sheepball".to_string(),
            level: 200,
            codes: vec![ILLEGAL_LEVEL, SUSPICIOUS_TALENT],
        });

        let anomalies = collector.finish();
        assert_eq!(anomalies.pal_count, 2);
        assert_eq!(anomalies.danger_count, 1);
        assert_eq!(
            anomalies
                .by_code
                .iter()
                .map(|entry| (entry.code, entry.count))
                .collect::<Vec<_>>(),
            vec![(SUSPICIOUS_TALENT, 2), (ILLEGAL_LEVEL, 1)]
        );
        assert_eq!(anomalies.flagged[0].severity, "warning");
        assert_eq!(anomalies.flagged[0].source, "dps");
        assert_eq!(
            anomalies.flagged[0].owner_uid,
            Some(uid("11111111-1111-1111-1111-111111111111"))
        );
        assert_eq!(anomalies.flagged[1].severity, "danger");
        assert_eq!(anomalies.flagged[1].source, "world");
        assert_eq!(anomalies.flagged[1].owner_uid, None);
        assert_eq!(anomalies.flagged[1].level, 200);
    }
}
