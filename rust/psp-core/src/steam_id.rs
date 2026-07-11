//! Port of palworld_save_pal/game/steam_id.py. All arithmetic is wrapping u32,
//! matching the Python _u32 mask-and-reinterpret helper.

#[derive(Debug, thiserror::Error)]
pub enum SteamIdError {
    #[error("Vanity URLs (/id/) are not supported. Use the numeric Steam ID from the profile URL (/profiles/...) instead.")]
    VanityUrl,
    #[error("")]
    NotNumeric,
    #[error("invalid uuid")]
    BadUuid,
}

pub fn parse_steam_input(raw: &str) -> Result<u64, SteamIdError> {
    let mut text = raw.trim().to_string();
    if let Some(rest) = text.split("steamcommunity.com/profiles/").nth(1) {
        text = rest.split('/').next().unwrap_or_default().to_string();
    } else if text.contains("steamcommunity.com/id/") {
        return Err(SteamIdError::VanityUrl);
    } else if let Some(stripped) = text.strip_prefix("steam_") {
        text = stripped.to_string();
    }
    text.parse::<u64>().map_err(|_| SteamIdError::NotNumeric)
}

fn is_hex_32(raw: &str) -> bool {
    raw.len() == 32 && raw.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_dashed_uuid(raw: &str) -> bool {
    let parts: Vec<&str> = raw.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(&parts)
            .all(|(len, part)| part.len() == *len && part.chars().all(|c| c.is_ascii_hexdigit()))
}

pub fn is_palworld_uid(raw: &str) -> bool {
    let trimmed = raw.trim();
    is_hex_32(trimmed) || is_dashed_uuid(trimmed)
}

pub fn parse_palworld_uid(raw: &str) -> Result<uuid::Uuid, SteamIdError> {
    uuid::Uuid::parse_str(raw.trim()).map_err(|_| SteamIdError::BadUuid)
}

/// CityHash64 over the UTF-16-LE encoding of the decimal steam id (steam_id.py:47-50).
pub fn steam_id_to_player_uid(steam_id: u64) -> uuid::Uuid {
    let decimal = steam_id.to_string();
    let utf16_le: Vec<u8> = decimal
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    let hash: u64 = cityhasher::hash(&utf16_le);
    let low = hash as u32;
    let high = hash >> 32;
    let uid_int = (low as u64).wrapping_add(high.wrapping_mul(23)) as u32;
    let mut bytes = [0u8; 16];
    bytes[..4].copy_from_slice(&uid_int.to_le_bytes());
    uuid::Uuid::from_bytes(bytes)
}

/// steam_id.py:53-63 — obfuscation cascade in wrapping u32 arithmetic.
pub fn player_uid_to_nosteam(player_uid: uuid::Uuid) -> String {
    let raw = u32::from_le_bytes(player_uid.as_bytes()[0..4].try_into().unwrap());
    let a = (raw << 8) ^ 2654435769u32.wrapping_sub(raw);
    let b = (a >> 13) ^ raw.wrapping_add(a).wrapping_neg();
    let c = (b >> 12) ^ raw.wrapping_sub(a).wrapping_sub(b);
    let d = (c << 16) ^ a.wrapping_sub(c).wrapping_sub(b);
    let e = (d >> 5) ^ b.wrapping_sub(d).wrapping_sub(c);
    let f = (e >> 3) ^ c.wrapping_sub(d).wrapping_sub(e);
    let g = (f << 10) ^ d.wrapping_sub(f).wrapping_sub(e);
    let result = (g >> 15) ^ e.wrapping_sub(g).wrapping_sub(f);
    format!("{result:08X}-0000-0000-0000-000000000000")
}
