use chrono::{NaiveDateTime, Timelike, Utc};

/// Mirrors Python `datetime.isoformat()` for naive datetimes: `T` separator,
/// 6-digit microseconds, fraction omitted entirely when microsecond == 0.
pub fn iso_naive(value: NaiveDateTime) -> String {
    if value.nanosecond() % 1_000_000_000 == 0 {
        value.format("%Y-%m-%dT%H:%M:%S").to_string()
    } else {
        value.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
    }
}

/// Mirrors Python `datetime.utcnow().isoformat()` (used for created_at columns).
pub fn now_iso_naive_utc() -> String {
    iso_naive(Utc::now().naive_utc())
}

/// Mirrors Python `datetime.now(timezone.utc).isoformat()` (used for updated_at /
/// last_accessed_at writes) — same string plus a `+00:00` suffix.
pub fn now_iso_utc_offset() -> String {
    format!("{}+00:00", iso_naive(Utc::now().naive_utc()))
}
