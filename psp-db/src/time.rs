use chrono::{NaiveDateTime, Timelike, Utc};

/// The timestamp shape every datetime column in this DB holds: `T` separator, 6-digit
/// microseconds, and no fraction at all when the microsecond component is zero.
pub fn iso_naive(value: NaiveDateTime) -> String {
    if value.nanosecond() % 1_000_000_000 == 0 {
        value.format("%Y-%m-%dT%H:%M:%S").to_string()
    } else {
        value.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
    }
}

/// Offset-free UTC now; what `created_at` columns store.
pub fn now_iso_naive_utc() -> String {
    iso_naive(Utc::now().naive_utc())
}

/// The same instant with a `+00:00` suffix; what `updated_at` and `last_accessed_at`
/// store, so those columns are not byte-comparable with `created_at`.
pub fn now_iso_utc_offset() -> String {
    format!("{}+00:00", iso_naive(Utc::now().naive_utc()))
}
