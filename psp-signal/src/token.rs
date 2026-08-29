//! Pairing-token generation and constant-time comparison.
//!
//! The alphabet is "unambiguous characters only" (no 0/O/1/I/L), the
//! length is 12 (~60 bits), and comparison is constant-time to resist
//! timing attacks.

/// 31 visually unambiguous characters. No `0/O`, `1/I/L`.
pub const TOKEN_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

pub const TOKEN_LEN: usize = 12;

/// Generates a token from OS randomness. `uuid::Uuid::new_v4` is backed by
/// the platform CSPRNG (`getrandom`), so the mapping onto the alphabet keeps
/// full uniformity by rejection-sampling bytes below the largest multiple of
/// the alphabet size.
pub fn generate_token() -> String {
    let mut out = String::with_capacity(TOKEN_LEN);
    while out.len() < TOKEN_LEN {
        let uuid = uuid::Uuid::new_v4();
        for &byte in uuid.as_bytes() {
            // 248 is the largest multiple of 31 that fits in a u8; modulo of
            // rejected bytes would bias the alphabet, so those are dropped.
            if byte < 248 {
                out.push(TOKEN_ALPHABET[(byte % 31) as usize] as char);
                if out.len() == TOKEN_LEN {
                    break;
                }
            }
        }
    }
    out
}

/// Constant-time equality: every byte of both strings is always folded in,
/// and the length mismatch contributes to the same accumulator, so timing
/// does not leak the position of the first difference (or the length).
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let max = a.len().max(b.len());
    let mut diff: u8 = (a.len() ^ b.len()) as u8;
    for i in 0..max {
        let byte_a = if i < a.len() { a[i] } else { 0 };
        let byte_b = if i < b.len() { b[i] } else { 0 };
        diff |= byte_a ^ byte_b;
    }
    diff == 0
}

/// True when every character comes from the token alphabet — persisted or
/// hand-edited tokens that contain anything else are refused on load.
pub fn is_well_formed(token: &str) -> bool {
    token.len() == TOKEN_LEN
        && token
            .bytes()
            .all(|byte| TOKEN_ALPHABET.contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generated_tokens_use_only_the_alphabet_and_requested_length() {
        for _ in 0..64 {
            let token = generate_token();
            assert_eq!(token.len(), TOKEN_LEN);
            assert!(is_well_formed(&token), "{token} must be alphabet-only");
        }
    }

    #[test]
    fn generated_tokens_vary() {
        let seen: HashSet<String> = (0..32).map(|_| generate_token()).collect();
        assert_eq!(seen.len(), 32, "tokens must not repeat");
    }

    #[test]
    fn constant_time_eq_matches_plain_semantics() {
        assert!(constant_time_eq("GSM4JNUD", "GSM4JNUD"));
        assert!(constant_time_eq("", ""));
        assert!(!constant_time_eq("GSM4JNUD", "GSM4JNUX"));
        assert!(!constant_time_eq("GSM4JNUD", "GSM4JNUD2"));
        assert!(!constant_time_eq("", "x"));
    }

    #[test]
    fn well_formed_rejects_ambiguous_or_wrong_length_tokens() {
        assert!(!is_well_formed("GSM4JNU")); // too short
        assert!(!is_well_formed("0O1ILZ9ABCD")); // ambiguous chars
        assert!(!is_well_formed("GSM4JNUD!")); // punctuation
    }
}
