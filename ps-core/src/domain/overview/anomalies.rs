//! "Pals needing review" collection: every pal the legality validator flags,
//! with per-code tallies and danger/warning severities. Mirrors the reference
//! project's anomalies block (`pal_count`, `danger_count`, `by_code`,
//! preview rows) — except the full flagged list travels on the wire and the
//! dashboard previews it client-side.

use std::collections::HashMap;

use crate::dto::overview::{OverviewAnomalies, OverviewAnomalyRow, OverviewCodeCount};

use super::illegal_pals::severity_of;

pub(crate) const SOURCE_WORLD: &str = "world";
pub(crate) const SOURCE_DPS: &str = "dps";

/// `codes` is the validator's non-empty result; severity is `"danger"` when
/// any code is a danger code, else `"warning"`.
pub(crate) fn flagged_row(
    instance_id: uuid::Uuid,
    owner_uid: Option<uuid::Uuid>,
    source: &'static str,
    character_id: &str,
    character_key: String,
    level: i64,
    codes: Vec<&'static str>,
) -> OverviewAnomalyRow {
    let is_danger = codes.iter().any(|code| severity_of(code) == "danger");
    OverviewAnomalyRow {
        instance_id,
        character_id: character_id.to_string(),
        character_key,
        level,
        severity: if is_danger { "danger" } else { "warning" },
        codes,
        owner_uid,
        source,
    }
}

pub(crate) struct AnomalyCollector {
    by_code: HashMap<&'static str, i64>,
    code_order: Vec<&'static str>,
    flagged: Vec<OverviewAnomalyRow>,
    danger_count: i64,
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

    pub(crate) fn record(&mut self, row: OverviewAnomalyRow) {
        if row.severity == "danger" {
            self.danger_count += 1;
        }
        for code in &row.codes {
            if let Some(count) = self.by_code.get_mut(code) {
                *count += 1;
            } else {
                self.code_order.push(code);
                self.by_code.insert(code, 1);
            }
        }
        self.flagged.push(row);
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

    fn row(instance_id: u128, level: i64, codes: Vec<&'static str>) -> OverviewAnomalyRow {
        flagged_row(
            uuid::Uuid::from_u128(instance_id),
            None,
            SOURCE_WORLD,
            "Sheepball",
            "Sheepball".to_string(),
            level,
            codes,
        )
    }

    #[test]
    fn tallies_codes_ranks_by_count_and_marks_severity() {
        let mut collector = AnomalyCollector::new();
        collector.record(row(1, 50, vec![SUSPICIOUS_TALENT]));
        collector.record(row(2, 200, vec![ILLEGAL_LEVEL, SUSPICIOUS_TALENT]));

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
        assert_eq!(anomalies.flagged[1].severity, "danger");
        assert_eq!(anomalies.flagged[1].level, 200);
    }
}
