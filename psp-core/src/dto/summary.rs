//! Summary wire DTOs. Field declaration order is a wire contract: `serde`
//! serializes in declaration order and the frontend consumes this JSON as-is
//! over the WebSocket.

use chrono::Timelike;

/// ISO-8601 with second precision, plus a 6-digit fractional part only when
/// microseconds are non-zero.
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

/// Exists only so `PlayerDto` (a request *and* response shape) can derive
/// `Deserialize`; `last_online_time` is server-computed, so this never sees
/// real frontend input. It must round-trip what `Serialize` emits.
impl<'de> serde::Deserialize<'de> for IsoDateTime {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
        chrono::NaiveDateTime::parse_from_str(&raw, "%Y-%m-%dT%H:%M:%S%.f")
            .map(IsoDateTime)
            .map_err(serde::de::Error::custom)
    }
}

/// Converts .NET-style ticks (100 ns intervals since `0001-01-01 00:00:00`)
/// to a datetime.
///
/// The quotient/remainder split by 10_000_000 is load-bearing, not a
/// micro-optimization: `ticks as f64` is lossy for any value at or above 2^53
/// (~9.007e15, i.e. any date after roughly the year 1000) and would discard
/// low bits before the division even ran. Both halves of the split stay well
/// under 2^53 and so convert to `f64` exactly, yielding a correctly-rounded
/// quotient.
///
/// `ticks == 0` is valid input and returns the epoch itself
/// (`0001-01-01T00:00:00`). A wire value of `0` means "no timestamp"; callers
/// must filter it out themselves rather than expecting `None` back.
///
/// Returns `None` (rather than panicking) if the resulting date falls
/// outside the range `chrono::NaiveDate` can represent.
pub fn ticks_to_datetime(ticks: u64) -> Option<chrono::NaiveDateTime> {
    let whole_seconds = ticks / 10_000_000;
    let tick_remainder = ticks % 10_000_000;
    let total_seconds = whole_seconds as f64 + (tick_remainder as f64) / 10_000_000.0;
    let day_count = (total_seconds / 86_400.0).floor() as i64;
    let seconds_remainder = total_seconds % 86_400.0;

    // Microseconds round half-to-even, carrying into seconds on a round-up to
    // a full second.
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

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PalSummary {
    pub instance_id: uuid::Uuid,
    pub character_id: String,
    pub character_key: String,
    pub nickname: Option<String>,
    pub owner_uid: Option<uuid::Uuid>,
    pub owner_name: Option<String>,
    pub guild_id: Option<uuid::Uuid>,
    pub base_id: Option<uuid::Uuid>,
    pub gender: Option<String>,
    pub level: i64,
    pub hp: i64,
    pub stomach: f64,
    pub rank: i64,
    pub exp: i64,
    pub talent_hp: i64,
    pub talent_shot: i64,
    pub talent_defense: i64,
    pub rank_hp: i64,
    pub rank_attack: i64,
    pub rank_defense: i64,
    pub rank_craftspeed: i64,
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
    fn test_ticks_to_isoformat_formats_correctly() {
        assert_eq!("2024-01-04T21:20:00", iso(638400000000000000));
        assert_eq!("2025-04-15T23:40:12.345680", iso(638803572123456789));
        assert_eq!("1970-01-01T00:00:00", iso(621355968000000000));
    }

    #[test]
    fn test_ticks_to_isoformat_preserves_sub_second_precision() {
        // Each tick value below is one where a naive `ticks as f64` division
        // drifts by several microseconds; they guard the quotient/remainder
        // split in `ticks_to_datetime`.
        assert_eq!("2026-04-07T16:36:46.740997", iso(639111766067410000));
        assert_eq!("1257-01-07T22:18:15.775871", iso(396361666957758680));
        assert_eq!("3231-11-10T06:23:14.035370", iso(1019559973940353740));
    }

    #[test]
    fn test_zero_ticks_is_year_one_epoch() {
        // The "0 ticks means no timestamp" guard belongs to callers, not here.
        assert_eq!(
            chrono::NaiveDate::from_ymd_opt(1, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0),
            ticks_to_datetime(0)
        );
    }

    #[test]
    fn test_absurd_ticks_does_not_panic() {
        // The guarantee under test is "no panic", not a specific date.
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

    #[test]
    fn test_iso_date_time_round_trips_through_its_own_wire_format() {
        let no_fraction = ticks_to_datetime(621355968000000000).unwrap();
        let serialized = serde_json::to_string(&IsoDateTime(no_fraction)).unwrap();
        let parsed: IsoDateTime = serde_json::from_str(&serialized).unwrap();
        assert_eq!(no_fraction, parsed.0);

        let with_fraction = ticks_to_datetime(638803572123456789).unwrap();
        let serialized = serde_json::to_string(&IsoDateTime(with_fraction)).unwrap();
        assert_eq!("\"2025-04-15T23:40:12.345680\"", serialized);
        let parsed: IsoDateTime = serde_json::from_str(&serialized).unwrap();
        assert_eq!(with_fraction, parsed.0);
    }

    #[test]
    fn test_pal_summary_wire_shape() {
        let summary = PalSummary {
            instance_id: "11111111-2222-3333-4444-555555555555".parse().unwrap(),
            character_id: "SheepBall".to_string(),
            character_key: "sheepball".to_string(),
            nickname: Some("Wooly".to_string()),
            owner_uid: Some("99999999-2222-3333-4444-555555555555".parse().unwrap()),
            owner_name: Some("Tester".to_string()),
            guild_id: None,
            base_id: None,
            gender: Some("Female".to_string()),
            level: 10,
            hp: 545000,
            stomach: 150.0,
            rank: 1,
            exp: 1000,
            talent_hp: 50,
            talent_shot: 50,
            talent_defense: 50,
            rank_hp: 0,
            rank_attack: 0,
            rank_defense: 0,
            rank_craftspeed: 0,
        };
        let serialized = serde_json::to_string(&summary).unwrap();
        assert_eq!(
            concat!(
                "{\"instance_id\":\"11111111-2222-3333-4444-555555555555\",",
                "\"character_id\":\"SheepBall\",",
                "\"character_key\":\"sheepball\",",
                "\"nickname\":\"Wooly\",",
                "\"owner_uid\":\"99999999-2222-3333-4444-555555555555\",",
                "\"owner_name\":\"Tester\",",
                "\"guild_id\":null,",
                "\"base_id\":null,",
                "\"gender\":\"Female\",",
                "\"level\":10,",
                "\"hp\":545000,",
                "\"stomach\":150.0,",
                "\"rank\":1,",
                "\"exp\":1000,",
                "\"talent_hp\":50,",
                "\"talent_shot\":50,",
                "\"talent_defense\":50,",
                "\"rank_hp\":0,",
                "\"rank_attack\":0,",
                "\"rank_defense\":0,",
                "\"rank_craftspeed\":0}"
            ),
            serialized
        );
    }
}
