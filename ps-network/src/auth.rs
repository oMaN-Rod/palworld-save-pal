//! PIN verification and session tokens.
//!
//! PBKDF2-HMAC-SHA256 is implemented directly on the `hmac`/`sha2` crates
//! (both already in the tree) rather than pulling a KDF dependency: it is
//! ~20 lines of well-understood loop, and it keeps the crate dependency
//! surface minimal. Comparison of derived keys is constant-time.
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rand::RngCore;
use sha2::digest::KeyInit;
use sha2::Digest;

/// OWASP 2023 recommends >= 600k iterations for PBKDF2-HMAC-SHA256; this
/// runs once per unlock attempt, not per request, so the latency is fine.
pub const PIN_ITERATIONS: u32 = 600_000;
pub const MIN_PIN_ITERATIONS: u32 = PIN_ITERATIONS;
pub const MAX_PIN_ITERATIONS: u32 = 1_200_000;

const SESSION_TOKEN_BYTES: usize = 32;
/// Hard cap so a leaked token cannot outlive its config'd TTL by much.
const MAX_SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const MAX_SESSIONS: usize = 1024;

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
        let now = Instant::now();
        let mut sessions = self
            .sessions
            .lock()
            .expect("session registry mutex poisoned");
        sessions.retain(|_, expiry| *expiry > now);
        while sessions.len() >= MAX_SESSIONS {
            if let Some(oldest) = sessions.keys().next().cloned() {
                sessions.remove(&oldest);
            } else {
                break;
            }
        }
        sessions.insert(token.clone(), now + ttl);
        token
    }

    pub fn is_valid(&self, token: &str) -> bool {
        let now = Instant::now();
        let mut sessions = self
            .sessions
            .lock()
            .expect("session registry mutex poisoned");
        sessions.retain(|_, expiry| *expiry > now);
        sessions.get(token).is_some_and(|expiry| *expiry > now)
    }

    pub fn revoke(&self, token: &str) {
        self.sessions
            .lock()
            .expect("session registry mutex poisoned")
            .remove(token);
    }

    /// Revoke every in-memory session after an authentication or access-policy
    /// change. Persisted configuration never contains session tokens, so a
    /// full clear is both deterministic and the safest response to a policy
    /// transition.
    pub fn revoke_all(&self) {
        self.sessions
            .lock()
            .expect("session registry mutex poisoned")
            .clear();
    }
}

const AUTH_WINDOW: Duration = Duration::from_secs(5 * 60);
const AUTH_LOCKOUT: Duration = Duration::from_secs(60);
const AUTH_FAILURE_LIMIT: u32 = 5;
const MAX_RATE_LIMIT_KEYS: usize = 4096;

#[derive(Debug, Clone, Copy)]
struct AttemptState {
    window_started: Instant,
    failures: u32,
    locked_until: Option<Instant>,
}

/// Bounded per-peer online PIN protection. Failed guesses never allocate
/// sessions and stale peer entries are pruned on every access.
#[derive(Default)]
pub struct AuthRateLimiter {
    attempts: Mutex<HashMap<IpAddr, AttemptState>>,
}

impl AuthRateLimiter {
    pub fn retry_after(&self, peer: IpAddr) -> Option<Duration> {
        let peer = crate::policy::canonical(peer);
        let now = Instant::now();
        let mut attempts = self.attempts.lock().expect("auth limiter mutex poisoned");
        Self::prune_locked(&mut attempts, now);
        attempts
            .get(&peer)
            .and_then(|state| state.locked_until)
            .and_then(|until| until.checked_duration_since(now))
    }

    pub fn record_failure(&self, peer: IpAddr) -> Option<Duration> {
        let peer = crate::policy::canonical(peer);
        let now = Instant::now();
        let mut attempts = self.attempts.lock().expect("auth limiter mutex poisoned");
        Self::prune_locked(&mut attempts, now);
        let retry_after = {
            let state = attempts.entry(peer).or_insert(AttemptState {
                window_started: now,
                failures: 0,
                locked_until: None,
            });
            if let Some(until) = state.locked_until {
                if until > now {
                    until.checked_duration_since(now)
                } else {
                    state.locked_until = None;
                    state.failures = 0;
                    state.window_started = now;
                    None
                }
            } else {
                None
            }
            .or_else(|| {
                if now.duration_since(state.window_started) >= AUTH_WINDOW {
                    state.window_started = now;
                    state.failures = 0;
                }
                state.failures = state.failures.saturating_add(1);
                if state.failures >= AUTH_FAILURE_LIMIT {
                    state.locked_until = Some(now + AUTH_LOCKOUT);
                    Some(AUTH_LOCKOUT)
                } else {
                    None
                }
            })
        };
        while attempts.len() > MAX_RATE_LIMIT_KEYS {
            if let Some(evicted) = attempts
                .keys()
                .copied()
                .find(|candidate| *candidate != peer)
            {
                attempts.remove(&evicted);
            } else {
                break;
            }
        }
        retry_after
    }

    pub fn record_success(&self, peer: IpAddr) {
        let peer = crate::policy::canonical(peer);
        self.attempts
            .lock()
            .expect("auth limiter mutex poisoned")
            .remove(&peer);
    }

    fn prune_locked(attempts: &mut HashMap<IpAddr, AttemptState>, now: Instant) {
        attempts.retain(|_, state| {
            state.locked_until.is_some_and(|until| until > now)
                || now.duration_since(state.window_started) < AUTH_WINDOW
        });
        while attempts.len() > MAX_RATE_LIMIT_KEYS {
            if let Some(peer) = attempts.keys().next().copied() {
                attempts.remove(&peer);
            } else {
                break;
            }
        }
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
    fn revoke_all_invalidates_every_issued_session() {
        let registry = SessionRegistry::default();
        let first = registry.issue(Duration::from_secs(60));
        let second = registry.issue(Duration::from_secs(60));
        registry.revoke_all();
        assert!(!registry.is_valid(&first));
        assert!(!registry.is_valid(&second));
    }

    #[test]
    fn session_ttl_is_capped() {
        let registry = SessionRegistry::default();
        let token = registry.issue(Duration::from_secs(u64::MAX / 2));
        // Still valid now; the cap only guards absurd TTLs from config.
        assert!(registry.is_valid(&token));
    }
}
