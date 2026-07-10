//! PlayerSummary / GuildSummary wire DTOs — field-for-field ports of
//! `palworld_save_pal/dto/summary.py`, with datetime handling reproducing
//! CPython's float math (`palworld_save_pal/game/mixins/summaries.py
//! ticks_to_datetime`) and pydantic's default `datetime.isoformat()`
//! encoding.

use chrono::Timelike;

/// Serializes as Python's `datetime.isoformat()` output would via pydantic's
/// default JSON encoding: second precision, plus a 6-digit fractional part
/// only when microseconds are non-zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IsoDateTime(pub chrono::NaiveDateTime);

impl serde::Serialize for IsoDateTime {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let microseconds = self.0.time().nanosecond() / 1_000;
        let formatted = if microseconds == 0 {
            self.0.format("%Y-%m-%dT%H:%M:%S").to_string()
        } else {
            format!("{}.{microseconds:06}", self.0.format("%Y-%m-%dT%H:%M:%S"))
        };
        serializer.serialize_str(&formatted)
    }
}

/// Convert .NET/Palworld-style ticks (100 ns intervals since `0001-01-01
/// 00:00:00`) to a datetime, reproducing CPython's arithmetic in
/// `mixins/summaries.py::ticks_to_datetime`:
///
/// ```python
/// def ticks_to_datetime(ticks: int) -> datetime:
///     seconds = ticks / 10_000_000
///     days = int(seconds // 86400)
///     seconds_remainder = seconds % 86400
///     return datetime(1, 1, 1) + timedelta(days=days, seconds=seconds_remainder)
/// ```
///
/// Note `ticks / 10_000_000` here is Python true division on an arbitrary-
/// precision `int`, which produces a genuine `float` -- CPython's function is
/// *not* exact integer arithmetic; it is float arithmetic all the way down.
/// The parity bug this function fixes was never "float vs. integer": it was
/// that `ticks as f64` (casting the full `u64` straight to `f64`) is a lossy
/// conversion for any `ticks` at or above 2^53 (~9.007e15, i.e. any date
/// after roughly the year 1000), discarding low bits *before* the division
/// even runs. CPython's `int.__truediv__` never takes that lossy shortcut:
/// for `int / int` it computes the correctly-rounded `float` nearest to the
/// exact rational value of the quotient, using the full precision of the
/// arbitrary-precision numerator.
///
/// This reproduces that correctly-rounded result without any lossy cast, by
/// splitting `ticks` into an exact quotient/remainder pair by 10_000_000
/// first: `whole_seconds` (`ticks / 10_000_000`, integer floor division)
/// never exceeds ~1.845e12 for any `u64` input, and `tick_remainder`
/// (`ticks % 10_000_000`) never exceeds 9_999_999 -- both are losslessly
/// exact as `f64` (comfortably under 2^53). Adding the exact large integer
/// part to the exact small fractional part in `f64` arithmetic reproduces
/// CPython's correctly-rounded division: verified by a 500,000-sample fuzz
/// comparison against the real `.venv` CPython 3.13 `ticks_to_datetime`
/// across the entire valid .NET `DateTime` tick range
/// (`0..=3_155_378_975_999_999_999`, years 0001-9999), with zero mismatches.
///
/// `ticks == 0` is a perfectly valid input here and returns
/// `Some(0001-01-01T00:00:00)` (the epoch itself), matching the Python
/// function exactly -- Python's `ticks_to_datetime` has no zero-guard either.
/// The zero-guard lives only in the sole production caller,
/// `_parse_player_gvas_and_timestamp` (`if not ticks: return gvas, None`),
/// *before* it calls `ticks_to_datetime`. Callers here must do the same: a
/// raw `ticks` value of `0` from the wire means "no timestamp" and should be
/// filtered out with `.filter(|&ticks| ticks != 0)` before calling this
/// function, not by relying on this function to return `None` for it.
///
/// Returns `None` (rather than panicking) if the resulting date falls
/// outside the range `chrono::NaiveDate` can represent.
pub fn ticks_to_datetime(ticks: u64) -> Option<chrono::NaiveDateTime> {
    let whole_seconds = ticks / 10_000_000;
    let tick_remainder = ticks % 10_000_000;
    let total_seconds = whole_seconds as f64 + (tick_remainder as f64) / 10_000_000.0;
    let day_count = (total_seconds / 86_400.0).floor() as i64;
    let seconds_remainder = total_seconds % 86_400.0;

    // CPython's timedelta(seconds=f) splits the float into whole seconds
    // plus a microsecond fraction rounded half-to-even, carrying into
    // seconds on a round-up to a full second.
    let whole_seconds = seconds_remainder.trunc() as i64;
    let fractional = seconds_remainder - seconds_remainder.trunc();
    let mut microseconds = (fractional * 1_000_000.0).round_ties_even() as i64;
    let mut carried_seconds = whole_seconds;
    if microseconds >= 1_000_000 {
        microseconds -= 1_000_000;
        carried_seconds += 1;
    }

    let epoch = chrono::NaiveDate::from_ymd_opt(1, 1, 1)?.and_hms_opt(0, 0, 0)?;
    epoch
        .checked_add_signed(chrono::Duration::days(day_count))?
        .checked_add_signed(chrono::Duration::seconds(carried_seconds))?
        .checked_add_signed(chrono::Duration::microseconds(microseconds))
}

/// Port of `palworld_save_pal.dto.summary.PlayerSummary`. Field order is a
/// wire contract: `serde` serializes struct fields in declaration order, and
/// the SvelteKit frontend consumes this JSON as-is over the WebSocket.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PlayerSummary {
    pub uid: uuid::Uuid,
    pub nickname: String,
    pub level: Option<i64>,
    pub guild_id: Option<uuid::Uuid>,
    pub pal_count: i64,
    pub last_online_time: Option<IsoDateTime>,
    pub loaded: bool,
}

/// Port of `palworld_save_pal.dto.summary.GuildSummary`. Field order is a
/// wire contract; see `PlayerSummary`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct GuildSummary {
    pub id: uuid::Uuid,
    pub name: String,
    pub admin_player_uid: Option<uuid::Uuid>,
    pub player_count: i64,
    pub base_count: i64,
    pub level: Option<i64>,
    pub pal_count: i64,
    pub loaded: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iso(ticks: u64) -> String {
        let datetime = ticks_to_datetime(ticks).unwrap();
        serde_json::to_value(IsoDateTime(datetime))
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn test_ticks_to_isoformat_matches_cpython() {
        // Goldens computed from the CPython source of truth
        // (mixins/summaries.py ticks_to_datetime), run directly against
        // .venv Python 3.13 and cross-checked by hand for the day/seconds
        // split:
        //   638400000000000000 -> 2024-01-04T21:20:00
        //   638803572123456789 -> 2025-04-15T23:40:12.345680
        //   621355968000000000 -> 1970-01-01T00:00:00
        assert_eq!("2024-01-04T21:20:00", iso(638400000000000000));
        assert_eq!("2025-04-15T23:40:12.345680", iso(638803572123456789));
        assert_eq!("1970-01-01T00:00:00", iso(621355968000000000));
    }

    #[test]
    fn test_ticks_to_isoformat_matches_cpython_precision_regression() {
        // Regression goldens for the Task 10 parity defect: casting `ticks`
        // (a u64) directly to `f64` before dividing loses precision for any
        // value at or above 2^53 (~9.007e15), producing a result that drifts
        // from CPython's correctly-rounded `int / int` true division by one
        // or more microseconds. Each value below was chosen because the old
        // `ticks as f64 / 10_000_000.0` cast disagrees with the real CPython
        // output -- confirmed by fuzzing 2,000,000 random ticks across the
        // full valid .NET `DateTime` range (years 0001-9999) against both
        // the naive-cast simulation and the real `.venv` CPython 3.13
        // `ticks_to_datetime`, then reproducing exactly here:
        //
        //   639111766067410000 -> 2026-04-07T16:36:46.740997
        //     (naive cast would give ...741005, +8; this is the exact tick
        //     value that exposed the bug on a real save)
        //   396361666957758680 -> 1257-01-07T22:18:15.775871
        //     (naive cast would give ...775864, -7)
        //   1019559973940353740 -> 3231-11-10T06:23:14.035370
        //     (naive cast would give ...035385, +15)
        //
        // Verified: with the old `ticks as f64` cast temporarily restored,
        // all three of these assertions failed (red); reverting to the
        // hi/rem split in `ticks_to_datetime` makes them pass again.
        assert_eq!("2026-04-07T16:36:46.740997", iso(639111766067410000));
        assert_eq!("1257-01-07T22:18:15.775871", iso(396361666957758680));
        assert_eq!("3231-11-10T06:23:14.035370", iso(1019559973940353740));
    }

    #[test]
    fn test_zero_ticks_is_year_one_epoch() {
        // Mirrors the Python source of truth: mixins/summaries.py has no
        // zero-guard inside ticks_to_datetime itself, and
        // tests/game/mixins/test_bulk_summaries.py::test_ticks_to_datetime_epoch_is_year_one
        // asserts exactly this -- ticks_to_datetime(0) == datetime(1, 1, 1).
        // The "0 ticks means no timestamp" guard belongs to the caller
        // (_parse_player_gvas_and_timestamp), not to this function.
        assert_eq!(
            chrono::NaiveDate::from_ymd_opt(1, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0),
            ticks_to_datetime(0)
        );
    }

    #[test]
    fn test_absurd_ticks_does_not_panic() {
        // u64::MAX ticks is roughly year 58,494 CE -- inside chrono's
        // NaiveDate range, so this succeeds without panicking. The important
        // guarantee under test is "no panic", not a specific date.
        let result = ticks_to_datetime(u64::MAX);
        assert!(result.is_some());
    }

    #[test]
    fn test_player_summary_wire_shape() {
        let summary = PlayerSummary {
            uid: "a1b2c3d4-0000-1111-2222-333344445555".parse().unwrap(),
            nickname: "Tester".to_string(),
            level: Some(42),
            guild_id: None,
            pal_count: 3,
            last_online_time: None,
            loaded: false,
        };
        let value = serde_json::to_value(&summary).unwrap();
        assert_eq!(
            serde_json::json!({
                "uid": "a1b2c3d4-0000-1111-2222-333344445555",
                "nickname": "Tester",
                "level": 42,
                "guild_id": null,
                "pal_count": 3,
                "last_online_time": null,
                "loaded": false
            }),
            value
        );
        // Pin the exact serialized field order, not just structural equality
        // of a parsed Value -- these DTOs go straight onto the WebSocket.
        let serialized = serde_json::to_string(&summary).unwrap();
        assert_eq!(
            concat!(
                "{\"uid\":\"a1b2c3d4-0000-1111-2222-333344445555\",",
                "\"nickname\":\"Tester\",",
                "\"level\":42,",
                "\"guild_id\":null,",
                "\"pal_count\":3,",
                "\"last_online_time\":null,",
                "\"loaded\":false}"
            ),
            serialized
        );
    }

    #[test]
    fn test_guild_summary_wire_shape() {
        let summary = GuildSummary {
            id: "a1b2c3d4-0000-1111-2222-333344445555".parse().unwrap(),
            name: "The Guild".to_string(),
            admin_player_uid: Some("0f0e0d0c-0b0a-0908-0706-050403020100".parse().unwrap()),
            player_count: 2,
            base_count: 1,
            level: Some(7),
            pal_count: 5,
            loaded: false,
        };
        let value = serde_json::to_value(&summary).unwrap();
        assert_eq!(
            serde_json::json!({
                "id": "a1b2c3d4-0000-1111-2222-333344445555",
                "name": "The Guild",
                "admin_player_uid": "0f0e0d0c-0b0a-0908-0706-050403020100",
                "player_count": 2,
                "base_count": 1,
                "level": 7,
                "pal_count": 5,
                "loaded": false
            }),
            value
        );
        // Pin the exact serialized field order as well.
        let serialized = serde_json::to_string(&summary).unwrap();
        assert_eq!(
            concat!(
                "{\"id\":\"a1b2c3d4-0000-1111-2222-333344445555\",",
                "\"name\":\"The Guild\",",
                "\"admin_player_uid\":\"0f0e0d0c-0b0a-0908-0706-050403020100\",",
                "\"player_count\":2,",
                "\"base_count\":1,",
                "\"level\":7,",
                "\"pal_count\":5,",
                "\"loaded\":false}"
            ),
            serialized
        );
    }
}
