//! PIN verification and session tokens.
//!
//! PBKDF2-HMAC-SHA256 is implemented directly on the `hmac`/`sha2` crates
//! (both already in the tree) rather than pulling a KDF dependency: it is
//! ~20 lines of well-understood loop, and it keeps the crate dependency
//! surface minimal. Comparison of derived keys is constant-time.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rand::RngCore;
use sha2::digest::KeyInit;
use sha2::Digest;

/// OWASP 2023 recommends >= 600k iterations for PBKDF2-HMAC-SHA256; this
/// runs once per unlock attempt, not per request, so the latency is fine.
pub const PIN_ITERATIONS: u32 = 600_000;

const SESSION_TOKEN_BYTES: usize = 32;
/// Hard cap so a leaked token cannot outlive its config'd TTL by much.
const MAX_SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

pub fn pbkdf2_hmac_sha256(secret: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    // PBKDF2 (RFC 8018) with one 32-byte block: SHA-256 output == block size,
    // so a single iteration chain covers the whole derived key.
    type HmacSha256 = hmac::Hmac<sha2::Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    hmac::Mac::update(&mut mac, salt);
    hmac::Mac::update(&mut mac, &1u32.to_be_bytes());
    let mut u = hmac::Mac::finalize(mac).into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&u);
    for _ in 1..iterations {
        let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
        hmac::Mac::update(&mut mac, &u);
        u = hmac::Mac::finalize(mac).into_bytes();
        for (o, b) in out.iter_mut().zip(u.iter()) {
            *o ^= b;
        }
    }
    out
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(
        s.as_bytes()
            .chunks(2)
            .map(|pair| {
                let hi = (pair[0] as char).to_digit(16).unwrap() as u8;
                let lo = (pair[1] as char).to_digit(16).unwrap() as u8;
                (hi << 4) | lo
            })
            .collect(),
    )
}

pub fn new_session_token() -> String {
    let mut token = [0u8; SESSION_TOKEN_BYTES];
    rand::rng().fill_bytes(&mut token);
    hex(&token)
}

/// In-memory session registry. Tokens die with the process — a server
/// restart requires re-entering the PIN, which is the safe default for a
/// tool that edits save files.
#[derive(Default)]
pub struct SessionRegistry {
    sessions: Mutex<HashMap<String, Instant>>,
}

impl SessionRegistry {
    pub fn issue(&self, ttl: Duration) -> String {
        let token = new_session_token();
        let ttl = ttl.min(MAX_SESSION_TTL);
        self.retain_expired(Instant::now());
        self.sessions
            .lock()
            .expect("session registry mutex poisoned")
            .insert(token.clone(), Instant::now() + ttl);
        token
    }

    pub fn is_valid(&self, token: &str) -> bool {
        let now = Instant::now();
        self.retain_expired(now);
        self.sessions
            .lock()
            .expect("session registry mutex poisoned")
            .get(token)
            .is_some_and(|expiry| *expiry > now)
    }

    pub fn revoke(&self, token: &str) {
        self.sessions
            .lock()
            .expect("session registry mutex poisoned")
            .remove(token);
    }

    fn retain_expired(&self, now: Instant) {
        self.sessions
            .lock()
            .expect("session registry mutex poisoned")
            .retain(|_, expiry| *expiry > now);
    }
}

/// SHA-256 of arbitrary bytes — used for fingerprinting config for the UI
/// ("your PIN is set"), never for passwords.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&sha2::Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pbkdf2_matches_reference_vector() {
        // RFC 7914-style self-consistency plus the classic SHA-256 vectors
        // from RFC 6070's HMAC-SHA256 successor draft ( Password "password",
        // salt "salt", 1 and 2 iterations).
        let one = pbkdf2_hmac_sha256(b"password", b"salt", 1);
        assert_eq!(
            hex(&one),
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        );
        let two = pbkdf2_hmac_sha256(b"password", b"salt", 2);
        assert_eq!(
            hex(&two),
            "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43"
        );
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(
            unhex(&hex(&[0x00, 0xab, 0xff])).unwrap(),
            vec![0x00, 0xab, 0xff]
        );
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
    }

    #[test]
    fn constant_time_eq_behaves() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }

    #[test]
    fn sessions_expire_and_revoke() {
        let registry = SessionRegistry::default();
        let token = registry.issue(Duration::from_millis(50));
        assert!(registry.is_valid(&token));
        registry.revoke(&token);
        assert!(!registry.is_valid(&token));

        let token = registry.issue(Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(40));
        assert!(!registry.is_valid(&token));
    }

    #[test]
    fn session_ttl_is_capped() {
        let registry = SessionRegistry::default();
        let token = registry.issue(Duration::from_secs(u64::MAX / 2));
        // Still valid now; the cap only guards absurd TTLs from config.
        assert!(registry.is_valid(&token));
    }
}
