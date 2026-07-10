//! Guild raw-tail codec -- the byte-exact encoder for the Guild branch of
//! `PalGroupData::remaining_data` (`palworld_save_tools/rawdata/group.py`'s
//! `decode_bytes`/`encode_bytes`, the `EPalGroupType::Guild` arm). Phase 1's
//! `palbin::parse_guild_raw_tail` already reads this same byte range but
//! only keeps the four fields the summary screens need
//! (`base_camp_level`/`guild_name`/`admin_player_uid`/`players`); every
//! other field there is discarded. Phase 2's guild edits (rename, base camp
//! level, player removal, admin lookup) need to write the tail *back* into
//! a save, so `GuildTail` here keeps every field -- including the
//! `leading_bytes`/`unknown_1`/`unknown_2`/`trailing_bytes` runs Python
//! itself never decodes further -- so `to_bytes` can reproduce the original
//! bytes exactly.
//!
//! Reads go through `palbin::BlobReader`, the bounds-checked cursor Phase 1
//! already built and proved against a real save (every read is gated by one
//! `checked_add`, so a hostile length prefix can never panic or wrap). The
//! writer half is new: it is the literal field-for-field inverse of that
//! reader, verified in `tests/guild_tail.rs` both synthetically and against
//! real guild tails extracted from the fixture saves at
//! `tests/fixtures/saves/{world1,world2}`.

use crate::error::CoreError;
use crate::palbin::BlobReader;
use crate::props;
use uesave::games::palworld::PalGroupData;
use uesave::{MapEntry, Property, StructValue};

/// One guild member's slot inside the `players` `TArray` of the
/// Guild-branch tail (`group.py`'s `player_info_reader`/`player_info_writer`).
pub struct GuildPlayerInfo {
    pub player_uid: uuid::Uuid,
    pub last_online_real_time: i64,
    pub player_name: String,
}

/// The full byte-for-byte Guild-branch layout of
/// `PalGroupData::remaining_data`, starting immediately after
/// `individual_character_handle_ids` (which uesave's `PalGroupData::read`
/// already consumes). Field order, per `group.py`'s `decode_bytes`
/// (`EPalGroupType::Guild` arm): `org_type: u8`, `leading_bytes: [u8; 4]`,
/// `base_ids: TArray<guid>`, `unknown_1: i32`, `base_camp_level: i32`,
/// `map_object_instance_ids_base_camp_points: TArray<guid>`,
/// `guild_name: fstring`, `last_guild_name_modifier_player_uid: guid`,
/// `unknown_2: [u8; 4]`, `admin_player_uid: guid`,
/// `players: TArray<{player_uid: guid, last_online_real_time: i64, player_name: fstring}>`,
/// `trailing_bytes: [u8; 4]`, then EOF.
pub struct GuildTail {
    pub org_type: u8,
    pub leading_bytes: [u8; 4],
    pub base_ids: Vec<uuid::Uuid>,
    pub unknown_1: i32,
    pub base_camp_level: i32,
    pub map_object_instance_ids_base_camp_points: Vec<uuid::Uuid>,
    pub guild_name: String,
    pub last_guild_name_modifier_player_uid: uuid::Uuid,
    pub unknown_2: [u8; 4],
    pub admin_player_uid: uuid::Uuid,
    pub players: Vec<GuildPlayerInfo>,
    pub trailing_bytes: [u8; 4],
}

/// Adds a field name to a leaf read's error, alongside `BlobReader::take`'s
/// byte offset (mirrors the private `palbin::describe_field`, which is not
/// reachable from this module).
fn describe_field<T>(field: &'static str, result: Result<T, CoreError>) -> Result<T, CoreError> {
    result.map_err(|err| match err {
        CoreError::Parse(message) => CoreError::Parse(format!("{field}: {message}")),
        other => other,
    })
}

fn read_four_raw_bytes(reader: &mut BlobReader) -> Result<[u8; 4], CoreError> {
    let mut bytes = [0u8; 4];
    for byte in &mut bytes {
        *byte = reader.read_u8()?;
    }
    Ok(bytes)
}

/// The Palworld guid byte permutation `BlobReader::read_uuid` applies to
/// turn 16 raw little-endian bytes into RFC 4122 display order: each of the
/// four 4-byte groups is reversed in place (`[3,2,1,0, 7,6,5,4, 11,10,9,8,
/// 15,14,13,12]`). Reversing a 4-byte block twice restores it, so this same
/// function also converts a canonical `Uuid`'s bytes back to the raw order
/// this codec writes -- the write-side mirror of `read_uuid`.
fn shuffle_guid_bytes(raw: [u8; 16]) -> [u8; 16] {
    [
        raw[3], raw[2], raw[1], raw[0], raw[7], raw[6], raw[5], raw[4], raw[11], raw[10], raw[9],
        raw[8], raw[15], raw[14], raw[13], raw[12],
    ]
}

fn write_uuid(buffer: &mut Vec<u8>, value: uuid::Uuid) {
    buffer.extend_from_slice(&shuffle_guid_bytes(*value.as_bytes()));
}

fn write_uuid_tarray(buffer: &mut Vec<u8>, values: &[uuid::Uuid]) {
    // `unwrap_or(u32::MAX)` rather than a panic: this codec must never
    // panic on its way to producing save bytes. A guid list exceeding four
    // billion entries is unreachable in practice (it would itself be
    // larger than any real save), so this is a defensive clamp, not a
    // documented behavior.
    let count = u32::try_from(values.len()).unwrap_or(u32::MAX);
    buffer.extend_from_slice(&count.to_le_bytes());
    for value in values {
        write_uuid(buffer, *value);
    }
}

/// Unreal `fstring`, matching `FArchiveWriter.fstring`
/// (`palworld_save_tools/archive.py`) field-for-field:
/// * the empty string is a bare zero length prefix and nothing else --
///   checked *before* the ASCII branch, not folded into it (Python's
///   `if string == "": self.i32(0)` is its own branch, so an ASCII string
///   of length zero and the empty string are not written the same way
///   `write_fstring` must replicate this ordering exactly, or an empty
///   string would gain a spurious NUL byte);
/// * an ASCII string is a positive `len + 1` length prefix, the ASCII
///   bytes, then one NUL byte;
/// * anything else is UTF-16LE with a negative `-(code_units + 1)` length
///   prefix, the code units, then a two-byte NUL terminator.
fn write_fstring(buffer: &mut Vec<u8>, value: &str) {
    if value.is_empty() {
        buffer.extend_from_slice(&0i32.to_le_bytes());
        return;
    }
    if value.is_ascii() {
        let ascii_bytes = value.as_bytes();
        // See `write_uuid_tarray`: clamped, not panicking, for an
        // unreachable-in-practice multi-gigabyte string.
        let length = i32::try_from(ascii_bytes.len() + 1).unwrap_or(i32::MAX);
        buffer.extend_from_slice(&length.to_le_bytes());
        buffer.extend_from_slice(ascii_bytes);
        buffer.push(0);
    } else {
        let code_units: Vec<u16> = value.encode_utf16().collect();
        let length = i32::try_from(code_units.len() + 1)
            .map(|len| -len)
            .unwrap_or(i32::MIN);
        buffer.extend_from_slice(&length.to_le_bytes());
        for unit in &code_units {
            buffer.extend_from_slice(&unit.to_le_bytes());
        }
        buffer.extend_from_slice(&0u16.to_le_bytes());
    }
}

impl GuildTail {
    /// Parses the Guild-branch tail of `PalGroupData::remaining_data`. The
    /// blob is assumed to already be positioned at `org_type` -- uesave's
    /// `PalGroupData::read` has already consumed `group_id`, `group_name`,
    /// and `individual_character_handle_ids` before `remaining_data` is
    /// populated. Consumes every byte; returns `Err` (never panics) on
    /// truncated input, an oversized declared count, or unconsumed
    /// trailing bytes.
    pub fn parse(remaining_data: &[u8]) -> Result<Self, CoreError> {
        let mut reader = BlobReader::new(remaining_data);
        let org_type = describe_field("org_type", reader.read_u8())?;
        let leading_bytes = describe_field("leading_bytes", read_four_raw_bytes(&mut reader))?;
        let base_ids = describe_field("base_ids", reader.read_tarray(BlobReader::read_uuid))?;
        let unknown_1 = describe_field("unknown_1", reader.read_i32())?;
        let base_camp_level = describe_field("base_camp_level", reader.read_i32())?;
        let map_object_instance_ids_base_camp_points = describe_field(
            "map_object_instance_ids_base_camp_points",
            reader.read_tarray(BlobReader::read_uuid),
        )?;
        let guild_name = describe_field("guild_name", reader.read_string())?;
        let last_guild_name_modifier_player_uid =
            describe_field("last_guild_name_modifier_player_uid", reader.read_uuid())?;
        let unknown_2 = describe_field("unknown_2", read_four_raw_bytes(&mut reader))?;
        let admin_player_uid = describe_field("admin_player_uid", reader.read_uuid())?;
        let players = describe_field(
            "players",
            reader.read_tarray(|element_reader| {
                Ok(GuildPlayerInfo {
                    player_uid: describe_field("players[].player_uid", element_reader.read_uuid())?,
                    last_online_real_time: describe_field(
                        "players[].last_online_real_time",
                        element_reader.read_i64(),
                    )?,
                    player_name: describe_field(
                        "players[].player_name",
                        element_reader.read_string(),
                    )?,
                })
            }),
        )?;
        let trailing_bytes = describe_field("trailing_bytes", read_four_raw_bytes(&mut reader))?;

        if !reader.is_at_end() {
            return Err(CoreError::Parse(format!(
                "guild tail: {} unread byte(s) remain at offset {} (blob is {} byte(s) long)",
                remaining_data.len() - reader.position(),
                reader.position(),
                remaining_data.len()
            )));
        }

        Ok(GuildTail {
            org_type,
            leading_bytes,
            base_ids,
            unknown_1,
            base_camp_level,
            map_object_instance_ids_base_camp_points,
            guild_name,
            last_guild_name_modifier_player_uid,
            unknown_2,
            admin_player_uid,
            players,
            trailing_bytes,
        })
    }

    /// Serializes back to the exact byte layout `parse` reads: every
    /// integer little-endian, every `fstring` written per `write_fstring`,
    /// every guid written per `write_uuid` (the involution that inverts
    /// `BlobReader::read_uuid`). This is the write-back path a corrupted or
    /// partially-written result must never reach -- every field here is
    /// already in memory (`self`), so nothing past this point can fail.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.push(self.org_type);
        buffer.extend_from_slice(&self.leading_bytes);
        write_uuid_tarray(&mut buffer, &self.base_ids);
        buffer.extend_from_slice(&self.unknown_1.to_le_bytes());
        buffer.extend_from_slice(&self.base_camp_level.to_le_bytes());
        write_uuid_tarray(&mut buffer, &self.map_object_instance_ids_base_camp_points);
        write_fstring(&mut buffer, &self.guild_name);
        write_uuid(&mut buffer, self.last_guild_name_modifier_player_uid);
        buffer.extend_from_slice(&self.unknown_2);
        write_uuid(&mut buffer, self.admin_player_uid);
        let player_count = u32::try_from(self.players.len()).unwrap_or(u32::MAX);
        buffer.extend_from_slice(&player_count.to_le_bytes());
        for player in &self.players {
            write_uuid(&mut buffer, player.player_uid);
            buffer.extend_from_slice(&player.last_online_real_time.to_le_bytes());
            write_fstring(&mut buffer, &player.player_name);
        }
        buffer.extend_from_slice(&self.trailing_bytes);
        buffer
    }
}

/// `entry.value.GroupType`, as its fully qualified enum variant name (e.g.
/// `"EPalGroupType::Guild"`). `None` if `entry.value` isn't a user struct or
/// carries no `GroupType` field.
pub fn entry_group_type(entry: &MapEntry) -> Option<String> {
    let value_properties = props::struct_properties(&entry.value)?;
    props::get(value_properties, &["GroupType"])
        .and_then(props::as_enum)
        .map(str::to_string)
}

/// `entry.value.RawData`, decoded as `PalGroupData` -- the typed struct
/// whose `remaining_data` is this module's `GuildTail::parse` input, for a
/// `GroupSaveDataMap` entry only (only meaningful when `entry_group_type`
/// returns `Some("EPalGroupType::Guild")`, but does not check that itself).
pub fn entry_group_data(entry: &MapEntry) -> Option<&PalGroupData> {
    let value_properties = props::struct_properties(&entry.value)?;
    match props::get(value_properties, &["RawData"])? {
        Property::Struct(StructValue::PalGroupData(data)) => Some(data),
        _ => None,
    }
}

/// Mutable counterpart of `entry_group_data`, for Phase 2's guild-edit
/// write path (rename, base camp level, player removal, admin lookup) to
/// mutate `remaining_data` in place via `GuildTail::to_bytes`.
pub fn entry_group_data_mut(entry: &mut MapEntry) -> Option<&mut PalGroupData> {
    let value_properties = props::struct_props_mut(&mut entry.value)?;
    match props::get_mut(value_properties, &["RawData"])? {
        Property::Struct(StructValue::PalGroupData(data)) => Some(data),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_fstring_matches_python_archive_writer_empty_string() {
        // Python's FArchiveWriter.fstring: `string == ""` is its own
        // branch (`self.i32(0)`), producing 4 bytes total -- not the
        // 5-byte ascii-branch result (`i32(1)` + NUL) an empty string
        // would get if routed through the ascii branch instead.
        let mut buffer = Vec::new();
        write_fstring(&mut buffer, "");
        assert_eq!(buffer, 0i32.to_le_bytes().to_vec());
    }

    #[test]
    fn write_fstring_matches_python_archive_writer_ascii() {
        let mut buffer = Vec::new();
        write_fstring(&mut buffer, "Guild");
        let mut expected = 6i32.to_le_bytes().to_vec(); // len + NUL
        expected.extend_from_slice(b"Guild\0");
        assert_eq!(buffer, expected);
    }

    #[test]
    fn write_fstring_matches_python_archive_writer_utf16() {
        let mut buffer = Vec::new();
        write_fstring(&mut buffer, "ギ"); // one UTF-16 code unit
        let mut expected = (-2i32).to_le_bytes().to_vec(); // -(1 unit + NUL unit)
        expected.extend_from_slice(&0x30ae_u16.to_le_bytes());
        expected.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(buffer, expected);
    }

    #[test]
    fn shuffle_guid_bytes_is_an_involution() {
        let raw: [u8; 16] = (0u8..16).collect::<Vec<u8>>().try_into().unwrap();
        assert_eq!(shuffle_guid_bytes(shuffle_guid_bytes(raw)), raw);
    }

    #[test]
    fn write_uuid_inverts_blob_reader_read_uuid() {
        let raw: [u8; 16] = (0u8..16).collect::<Vec<u8>>().try_into().unwrap();
        let mut reader = BlobReader::new(&raw);
        let uuid = reader.read_uuid().unwrap();
        let mut buffer = Vec::new();
        write_uuid(&mut buffer, uuid);
        assert_eq!(buffer, raw);
    }

    #[test]
    fn parse_rejects_oversized_players_count_without_panicking() {
        let mut header = Vec::new();
        header.push(0u8); // org_type
        header.extend_from_slice(&[0; 4]); // leading_bytes
        header.extend_from_slice(&0u32.to_le_bytes()); // base_ids count
        header.extend_from_slice(&0i32.to_le_bytes()); // unknown_1
        header.extend_from_slice(&1i32.to_le_bytes()); // base_camp_level
        header.extend_from_slice(&0u32.to_le_bytes()); // base camp points count
        write_fstring(&mut header, "G");
        write_uuid(&mut header, uuid::Uuid::nil()); // last modifier
        header.extend_from_slice(&[0; 4]); // unknown_2
        write_uuid(&mut header, uuid::Uuid::nil()); // admin
        header.extend_from_slice(&u32::MAX.to_le_bytes()); // players count: absurd
                                                           // no player data follows
        assert!(GuildTail::parse(&header).is_err());
    }
}
