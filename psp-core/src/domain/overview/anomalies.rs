//! "Pals needing review" collection: every pal the legality validator flags,
//! with per-code tallies and danger/warning severities. Mirrors the reference
//! project's anomalies block (`pal_count`, `danger_count`, `by_code`,
//! preview rows) — except the full flagged list travels on the wire and the
//! dashboard previews it client-side.

use std::collections::HashMap;

use crate::dto::overview::{OverviewAnomalies, OverviewAnomalyRow, OverviewCodeCount};
use crate::ue::MapEntry;

use crate::domain::world;

use super::illegal_pals::severity_of;

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

    /// Records one flagged pal. `codes` is the validator's non-empty result;
    /// severity is `"danger"` when any code is a danger code, else
    /// `"warning"`.
    pub(crate) fn record(
        &mut self,
        entry: &MapEntry,
        character_id: &str,
        character_key: String,
        level: i64,
        codes: Vec<&'static str>,
    ) {
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
            instance_id: world::entry_instance_id(entry).unwrap_or(uuid::Uuid::nil()),
            character_id: character_id.to_string(),
            character_key,
            level,
            severity: if is_danger { "danger" } else { "warning" },
            codes,
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
    use crate::ue::{Properties, Property, StructValue};

    fn guid_property(text: &str) -> Property {
        Property::Struct(StructValue::Guid(
            serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap(),
        ))
    }

    fn entry(instance_id: &str) -> MapEntry {
        let mut key_properties = Properties::default();
        key_properties.insert(
            "PlayerUId",
            guid_property("00000000-0000-0000-0000-000000000000"),
        );
        key_properties.insert("InstanceId", guid_property(instance_id));
        MapEntry {
            key: Property::Struct(StructValue::Struct(key_properties)),
            value: Property::Struct(StructValue::Struct(Properties::default())),
        }
    }

    #[test]
    fn tallies_codes_ranks_by_count_and_marks_severity() {
        let mut collector = AnomalyCollector::new();
        // First pal: one warning code.
        collector.record(
            &entry("aaaaaaaa-0000-0000-0000-000000000001"),
            "Sheepball",
            "Sheepball".to_string(),
            50,
            vec![SUSPICIOUS_TALENT],
        );
        // Second pal: a danger + a warning code → danger severity.
        collector.record(
            &entry("aaaaaaaa-0000-0000-0000-000000000002"),
            "Sheepball",
            "Sheepball".to_string(),
            200,
            vec![ILLEGAL_LEVEL, SUSPICIOUS_TALENT],
        );

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
