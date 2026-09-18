use serde::Serialize;

pub const GAME_DOMAIN: &str = "palworld";
const MAX_KEY_LEN: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NxmLink {
    pub mod_id: u32,
    pub file_id: u32,
    pub key: Option<String>,
    pub expires: Option<u64>,
}

impl NxmLink {
    pub fn is_expired(&self, now_secs: u64) -> bool {
        self.expires.is_some_and(|expires| expires <= now_secs)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NxmError {
    #[error("this is not an nxm:// link")]
    NotNxm,
    #[error("this nxm:// link is for {0}, not Palworld")]
    OtherGame(String),
    #[error("collection links are not supported")]
    Collection,
    #[error("sign-in links are not handled here")]
    OAuth,
    #[error("this nxm:// link is malformed: {0}")]
    Malformed(&'static str),
}

impl NxmError {
    pub fn code(&self) -> &'static str {
        match self {
            NxmError::NotNxm | NxmError::Malformed(_) => "invalid_link",
            NxmError::OtherGame(_) | NxmError::Collection | NxmError::OAuth => "unsupported_link",
        }
    }
}

pub fn parse_nxm(input: &str) -> Result<NxmLink, NxmError> {
    let input = input.trim();
    let rest = input
        .get(..6)
        .filter(|scheme| scheme.eq_ignore_ascii_case("nxm://"))
        .map(|_| &input[6..])
        .ok_or(NxmError::NotNxm)?;
    let rest = rest.split('#').next().unwrap_or_default();
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    let (path, query) = match rest.split_once('?') {
        Some((path, query)) => (path, query),
        None => (rest, ""),
    };

    let segments: Vec<&str> = path.split('/').collect();
    let host = segments.first().copied().unwrap_or_default();
    if host.is_empty() {
        return Err(NxmError::Malformed("the game is missing"));
    }
    if host.eq_ignore_ascii_case("oauth") {
        return Err(NxmError::OAuth);
    }
    if !host.eq_ignore_ascii_case(GAME_DOMAIN) {
        return Err(NxmError::OtherGame(host.to_ascii_lowercase()));
    }
    let mut tail = &segments[1..];
    if tail.last() == Some(&"") {
        tail = &tail[..tail.len() - 1];
    }
    let (mod_id, file_id) = match tail {
        [first, ..] if first.eq_ignore_ascii_case("collections") => {
            return Err(NxmError::Collection)
        }
        [mods, mod_id, files, file_id]
            if mods.eq_ignore_ascii_case("mods") && files.eq_ignore_ascii_case("files") =>
        {
            (
                parse_id(mod_id).ok_or(NxmError::Malformed("the mod id is not a number"))?,
                parse_id(file_id).ok_or(NxmError::Malformed("the file id is not a number"))?,
            )
        }
        _ => return Err(NxmError::Malformed("expected mods/{id}/files/{id}")),
    };

    let mut key = None;
    let mut expires = None;
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = percent_decode(value).ok_or(NxmError::Malformed("bad percent encoding"))?;
        match name {
            "key" => {
                if value.len() > MAX_KEY_LEN || value.chars().any(char::is_control) {
                    return Err(NxmError::Malformed("the key is not usable"));
                }
                key = Some(value).filter(|value| !value.is_empty());
            }
            "expires" => {
                expires = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| NxmError::Malformed("expires is not a number"))?,
                );
            }
            _ => {}
        }
    }
    if key.is_some() != expires.is_some() {
        return Err(NxmError::Malformed("key and expires go together"));
    }
    Ok(NxmLink {
        mod_id,
        file_id,
        key,
        expires,
    })
}

fn parse_id(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u32>().ok().filter(|id| *id > 0)
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let hex = bytes.get(index + 1..index + 3)?;
                if !hex.iter().all(u8::is_ascii_hexdigit) {
                    return None;
                }
                out.push(u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?);
                index += 3;
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mod_manager_download_link_parses() {
        let link = parse_nxm(
            "nxm://palworld/mods/4821/files/99001?key=abc-DEF_1&expires=1700000000&user_id=7",
        )
        .unwrap();
        assert_eq!(
            link,
            NxmLink {
                mod_id: 4821,
                file_id: 99001,
                key: Some("abc-DEF_1".to_string()),
                expires: Some(1_700_000_000),
            }
        );
    }

    #[test]
    fn the_empty_authority_form_and_case_and_a_trailing_slash_are_accepted() {
        let link = parse_nxm("NXM:///PalWorld/Mods/4821/Files/99001/").unwrap();
        assert_eq!(
            (link.mod_id, link.file_id, link.key, link.expires),
            (4821, 99001, None, None)
        );
    }

    #[test]
    fn other_links_are_rejected_with_their_codes() {
        let cases = [
            ("https://www.nexusmods.com/palworld/mods/1", "invalid_link"),
            (
                "nxm://palworld/collections/tidy/revisions/2",
                "unsupported_link",
            ),
            ("nxm://oauth/callback?code=x", "unsupported_link"),
            (
                "nxm://skyrimspecialedition/mods/1/files/2",
                "unsupported_link",
            ),
            ("nxm://palworld/mods/4821/files/99001/extra", "invalid_link"),
            ("nxm://palworld/mods/abc/files/1", "invalid_link"),
            ("nxm://palworld/mods/0/files/1", "invalid_link"),
            ("nxm://palworld/mods/1/files/2?key=abc", "invalid_link"),
            ("nxm://palworld/mods/1/files/2?expires=5", "invalid_link"),
            (
                "nxm://palworld/mods/1/files/2?key=a&expires=soon",
                "invalid_link",
            ),
            (
                "nxm://palworld/mods/1/files/2?key=a%zz&expires=5",
                "invalid_link",
            ),
            ("nxm://", "invalid_link"),
        ];
        for (input, code) in cases {
            let error = parse_nxm(input).expect_err(input);
            assert_eq!(error.code(), code, "{input}: {error:?}");
        }
        assert_eq!(
            parse_nxm("nxm://skyrimspecialedition/mods/1/files/2"),
            Err(NxmError::OtherGame("skyrimspecialedition".to_string()))
        );
    }

    #[test]
    fn percent_encoded_keys_are_decoded() {
        let link = parse_nxm("nxm://palworld/mods/1/files/2?key=a%2Bb&expires=9#frag").unwrap();
        assert_eq!(link.key.as_deref(), Some("a+b"));
        assert_eq!(link.expires, Some(9));
    }

    #[test]
    fn expiry_is_inclusive_of_now() {
        let link = parse_nxm("nxm://palworld/mods/1/files/2?key=k&expires=100").unwrap();
        assert!(link.is_expired(100));
        assert!(!link.is_expired(99));
        assert!(!parse_nxm("nxm://palworld/mods/1/files/2")
            .unwrap()
            .is_expired(u64::MAX));
    }
}
