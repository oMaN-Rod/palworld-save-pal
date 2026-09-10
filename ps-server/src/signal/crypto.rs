//! Pairing code generation, HKDF key derivation, and AEAD sealed envelopes
//! for Signal pairing. The TypeScript port in `ps-ui/src/lib/signal/crypto.ts`
//! must stay byte-identical to this module; `vectors.json` locks the two
//! together.
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

const PAIRING_CODE_LEN: usize = 26;
const PAIRING_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
const HKDF_SALT: &[u8] = b"psp-signal-v1";
const NONCE_LEN: usize = 12;

pub struct PairingKeys {
    pub room_id: String,
    pub seal_key: [u8; 32],
}

pub fn generate_secret32() -> [u8; 32] {
    let mut secret = [0u8; 32];
    rand::rng().fill_bytes(&mut secret);
    secret
}

pub fn generate_device_id() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    hex_lower(&bytes)
}

pub fn generate_pairing_code() -> String {
    let alphabet_len = PAIRING_ALPHABET.len() as u32;
    let reject_at_or_above = 256 - (256 % alphabet_len);
    let mut rng = rand::rng();
    let mut code = String::with_capacity(PAIRING_CODE_LEN);
    let mut byte = [0u8; 1];
    while code.len() < PAIRING_CODE_LEN {
        rng.fill_bytes(&mut byte);
        let value = byte[0] as u32;
        if value < reject_at_or_above {
            code.push(PAIRING_ALPHABET[(value % alphabet_len) as usize] as char);
        }
    }
    code
}

pub fn normalize_code(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

pub fn derive_keys(code: &str) -> PairingKeys {
    let normalized = normalize_code(code);
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), normalized.as_bytes());

    let mut room_bytes = [0u8; 16];
    hk.expand(b"room", &mut room_bytes)
        .expect("16 is a valid HKDF-SHA256 output length");
    let mut seal_key = [0u8; 32];
    hk.expand(b"seal", &mut seal_key)
        .expect("32 is a valid HKDF-SHA256 output length");

    PairingKeys {
        room_id: hex_lower(&room_bytes),
        seal_key,
    }
}

pub fn derive_meet_room(identity_secret: &[u8; 32]) -> String {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), identity_secret);
    let mut room_bytes = [0u8; 16];
    hk.expand(b"meet-room", &mut room_bytes)
        .expect("16 is a valid HKDF-SHA256 output length");
    hex_lower(&room_bytes)
}

pub fn derive_device_seal_key(device_secret: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), device_secret);
    let mut seal_key = [0u8; 32];
    hk.expand(b"seal", &mut seal_key)
        .expect("32 is a valid HKDF-SHA256 output length");
    seal_key
}

pub fn hex32(bytes: &[u8; 32]) -> String {
    hex_lower(bytes)
}

pub fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").expect("writing to a String cannot fail");
    }
    out
}

pub fn seal(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Vec<u8> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce_bytes);
    seal_with_nonce(key, &nonce_bytes, plaintext, aad)
}

pub fn open(key: &[u8; 32], sealed: &[u8], aad: &[u8]) -> Result<Vec<u8>, ()> {
    if sealed.len() < NONCE_LEN {
        return Err(());
    }
    let (nonce_bytes, ciphertext) = sealed.split_at(NONCE_LEN);
    let nonce_bytes: [u8; NONCE_LEN] = nonce_bytes
        .try_into()
        .expect("split_at(NONCE_LEN) guarantees this length");
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let nonce = Nonce::from(nonce_bytes);
    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| ())
}

fn seal_with_nonce(
    key: &[u8; 32],
    nonce_bytes: &[u8; NONCE_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Vec<u8> {
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let nonce = Nonce::from(*nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("encryption with a valid 32-byte key and 12-byte nonce cannot fail");

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(nonce_bytes);
    out.extend_from_slice(&ciphertext);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn round_trips_through_seal_and_open() {
        let key = [7u8; 32];
        let plaintext = b"hello signal room";
        let sealed = seal(&key, plaintext, b"host");

        let opened = open(&key, &sealed, b"host").expect("valid envelope must open");

        assert_eq!(opened, plaintext);
    }

    #[test]
    fn nonce_is_random_per_call() {
        let key = [3u8; 32];
        let a = seal(&key, b"same plaintext", b"guest");
        let b = seal(&key, b"same plaintext", b"guest");

        assert_ne!(a[..NONCE_LEN], b[..NONCE_LEN]);
    }

    #[test]
    fn tampered_ciphertext_fails_to_open() {
        let key = [9u8; 32];
        let mut sealed = seal(&key, b"do not touch me", b"host");
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;

        assert_eq!(open(&key, &sealed, b"host"), Err(()));
    }

    #[test]
    fn wrong_aad_fails_to_open() {
        let key = [1u8; 32];
        let sealed = seal(&key, b"reflected message", b"host");

        assert_eq!(open(&key, &sealed, b"guest"), Err(()));
    }

    #[test]
    fn truncated_envelope_fails_to_open() {
        let key = [2u8; 32];
        assert_eq!(open(&key, &[0u8; 4], b"host"), Err(()));
    }

    #[test]
    fn derive_keys_is_deterministic() {
        let a = derive_keys("ABCD-EFGH-JKMN-PQRS-TUVW-XY23");
        let b = derive_keys("ABCD-EFGH-JKMN-PQRS-TUVW-XY23");

        assert_eq!(a.room_id, b.room_id);
        assert_eq!(a.seal_key, b.seal_key);
    }

    #[test]
    fn derive_keys_ignores_case_dashes_and_whitespace() {
        let a = derive_keys("ABCD-EFGH-JKMN-PQRS-TUVW-XY23");
        let b = derive_keys(" abcd efgh-jkmn pqrs-tuvw xy23 ");

        assert_eq!(a.room_id, b.room_id);
        assert_eq!(a.seal_key, b.seal_key);
    }

    #[test]
    fn derive_keys_room_id_is_32_lowercase_hex_chars() {
        let keys = derive_keys("SOME-PAIRING-CODE");

        assert_eq!(keys.room_id.len(), 32);
        assert!(keys
            .room_id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn different_codes_derive_different_keys() {
        let a = derive_keys("CODE-ONE-XXXXXX");
        let b = derive_keys("CODE-TWO-YYYYYY");

        assert_ne!(a.room_id, b.room_id);
        assert_ne!(a.seal_key, b.seal_key);
    }

    #[test]
    fn normalize_code_strips_dashes_whitespace_and_uppercases() {
        assert_eq!(normalize_code("abcd-1234 efgh"), "ABCD1234EFGH");
        assert_eq!(normalize_code("  xy-z  "), "XYZ");
        assert_eq!(normalize_code("ALREADY-NORMAL"), "ALREADYNORMAL");
    }

    #[test]
    fn generated_device_ids_are_16_bytes_of_hex_and_distinct() {
        let a = generate_device_id();
        let b = generate_device_id();

        assert_eq!(a.len(), 32);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_ne!(a, b);
    }

    #[test]
    fn generated_codes_have_the_expected_length_and_alphabet() {
        let forbidden = ['0', 'O', '1', 'I', 'L'];
        let mut seen = HashSet::new();

        for _ in 0..500 {
            let code = generate_pairing_code();
            assert_eq!(code.len(), PAIRING_CODE_LEN);
            for c in code.chars() {
                assert!(c.is_ascii_uppercase() || c.is_ascii_digit());
                assert!(
                    !forbidden.contains(&c),
                    "forbidden character {c} in code {code}"
                );
            }
            seen.insert(code);
        }

        assert_eq!(seen.len(), 500);
    }

    #[test]
    fn generated_codes_use_a_crypto_rng_not_a_fixed_pattern() {
        let a = generate_pairing_code();
        let b = generate_pairing_code();
        assert_ne!(a, b);
    }

    #[test]
    fn derive_meet_room_is_deterministic() {
        let secret = [5u8; 32];
        assert_eq!(derive_meet_room(&secret), derive_meet_room(&secret));
    }

    #[test]
    fn derive_meet_room_is_32_lowercase_hex_chars() {
        let room = derive_meet_room(&[6u8; 32]);
        assert_eq!(room.len(), 32);
        assert!(room
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn derive_meet_room_differs_for_distinct_secrets() {
        let a = derive_meet_room(&[1u8; 32]);
        let b = derive_meet_room(&[2u8; 32]);
        assert_ne!(a, b);
    }

    #[test]
    fn derive_device_seal_key_is_deterministic() {
        let secret = [8u8; 32];
        assert_eq!(
            derive_device_seal_key(&secret),
            derive_device_seal_key(&secret)
        );
    }

    #[test]
    fn device_sealed_round_trip() {
        let secret = generate_secret32();
        let key = derive_device_seal_key(&secret);
        let sealed = seal(&key, b"device message", b"guest");

        let opened = open(&key, &sealed, b"guest").expect("valid envelope must open");
        assert_eq!(opened, b"device message");

        assert_eq!(open(&key, &sealed, b"host"), Err(()));
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Vector {
        code: String,
        room_id: String,
        seal_key_hex: String,
        aad_role: String,
        nonce_hex: String,
        plaintext_hex: String,
        sealed_hex: String,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct DeviceSealVector {
        device_secret_hex: String,
        seal_key_hex: String,
        aad_role: String,
        nonce_hex: String,
        plaintext_hex: String,
        sealed_hex: String,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct MeetRoomVector {
        identity_secret_hex: String,
        meet_room: String,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct VectorsFile {
        pairing: Vec<Vector>,
        device_seal: Vec<DeviceSealVector>,
        meet_room: Vec<MeetRoomVector>,
    }

    fn decode_hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex vector fixture"))
            .collect()
    }

    fn load_vectors() -> VectorsFile {
        const RAW: &str = include_str!("../../../ps-ui/src/lib/signal/vectors.json");
        serde_json::from_str(RAW).expect("vectors.json must parse")
    }

    #[test]
    fn shared_vectors_conform_in_rust() {
        let vectors = load_vectors().pairing;
        assert!(!vectors.is_empty());

        for v in &vectors {
            let keys = derive_keys(&v.code);
            assert_eq!(
                keys.room_id, v.room_id,
                "room_id mismatch for code {}",
                v.code
            );
            assert_eq!(
                hex_lower(&keys.seal_key),
                v.seal_key_hex,
                "seal_key mismatch for code {}",
                v.code
            );

            let nonce_bytes: [u8; NONCE_LEN] = decode_hex(&v.nonce_hex)
                .try_into()
                .expect("12-byte nonce fixture");
            let plaintext = decode_hex(&v.plaintext_hex);
            let sealed = decode_hex(&v.sealed_hex);

            let resealed = seal_with_nonce(
                &keys.seal_key,
                &nonce_bytes,
                &plaintext,
                v.aad_role.as_bytes(),
            );
            assert_eq!(resealed, sealed, "reseal mismatch for code {}", v.code);

            let opened =
                open(&keys.seal_key, &sealed, v.aad_role.as_bytes()).expect("fixture must open");
            assert_eq!(
                opened, plaintext,
                "opened plaintext mismatch for code {}",
                v.code
            );
        }
    }

    #[test]
    fn device_seal_vectors_conform_in_rust() {
        let vectors = load_vectors().device_seal;
        assert!(!vectors.is_empty());

        for v in &vectors {
            let secret = parse_hex32(&v.device_secret_hex).expect("valid hex32 fixture");
            let key = derive_device_seal_key(&secret);
            assert_eq!(
                hex_lower(&key),
                v.seal_key_hex,
                "seal_key mismatch for device secret {}",
                v.device_secret_hex
            );

            let nonce_bytes: [u8; NONCE_LEN] = decode_hex(&v.nonce_hex)
                .try_into()
                .expect("12-byte nonce fixture");
            let plaintext = decode_hex(&v.plaintext_hex);
            let sealed = decode_hex(&v.sealed_hex);

            let resealed = seal_with_nonce(&key, &nonce_bytes, &plaintext, v.aad_role.as_bytes());
            assert_eq!(
                resealed, sealed,
                "reseal mismatch for device secret {}",
                v.device_secret_hex
            );

            let opened = open(&key, &sealed, v.aad_role.as_bytes()).expect("fixture must open");
            assert_eq!(
                opened, plaintext,
                "opened plaintext mismatch for device secret {}",
                v.device_secret_hex
            );
        }
    }

    #[test]
    fn meet_room_vectors_conform_in_rust() {
        let vectors = load_vectors().meet_room;
        assert!(!vectors.is_empty());

        for v in &vectors {
            let secret = parse_hex32(&v.identity_secret_hex).expect("valid hex32 fixture");
            assert_eq!(
                derive_meet_room(&secret),
                v.meet_room,
                "meet_room mismatch for identity secret {}",
                v.identity_secret_hex
            );
        }
    }

    /// Regenerates `vectors.json`. Not run by default:
    /// `cargo test -p ps-server --lib -- --ignored print_vectors_fixture --nocapture`
    /// prints fresh vectors to paste into the file after any change to the
    /// derivation or seal format.
    #[test]
    #[ignore]
    fn print_vectors_fixture() {
        struct Case {
            code: &'static str,
            aad_role: &'static str,
            nonce: [u8; NONCE_LEN],
            plaintext: &'static [u8],
        }

        struct DeviceCase {
            device_secret: [u8; 32],
            aad_role: &'static str,
            nonce: [u8; NONCE_LEN],
            plaintext: &'static [u8],
        }

        struct RoomCase {
            identity_secret: [u8; 32],
        }

        let cases = [
            Case {
                code: "ABCD-EFGH-JKMN-PQRS-TUVW-XY23",
                aad_role: "host",
                nonce: [0x01; NONCE_LEN],
                plaintext: b"hello from the host",
            },
            Case {
                code: "7Q2R-9TVW-XYZ2-3456-789A-BCDE",
                aad_role: "guest",
                nonce: [0x02; NONCE_LEN],
                plaintext: b"{}",
            },
            Case {
                code: "single-word-pairing-code-here",
                aad_role: "host",
                nonce: [0xAB; NONCE_LEN],
                plaintext:
                    b"a longer plaintext payload used to exercise multi-block AES-GCM sealing",
            },
        ];

        let device_cases = [
            DeviceCase {
                device_secret: [0x11; 32],
                aad_role: "guest",
                nonce: [0x03; NONCE_LEN],
                plaintext: b"device sealed payload one",
            },
            DeviceCase {
                device_secret: [0x22; 32],
                aad_role: "host",
                nonce: [0x04; NONCE_LEN],
                plaintext: b"{}",
            },
        ];

        let room_cases = [
            RoomCase {
                identity_secret: [0x33; 32],
            },
            RoomCase {
                identity_secret: [0x44; 32],
            },
        ];

        println!("{{");
        println!("  \"pairing\": [");
        for (i, case) in cases.iter().enumerate() {
            let keys = derive_keys(case.code);
            let sealed = seal_with_nonce(
                &keys.seal_key,
                &case.nonce,
                case.plaintext,
                case.aad_role.as_bytes(),
            );
            let comma = if i + 1 == cases.len() { "" } else { "," };
            println!("    {{");
            println!("      \"code\": \"{}\",", case.code);
            println!("      \"roomId\": \"{}\",", keys.room_id);
            println!("      \"sealKeyHex\": \"{}\",", hex_lower(&keys.seal_key));
            println!("      \"aadRole\": \"{}\",", case.aad_role);
            println!("      \"nonceHex\": \"{}\",", hex_lower(&case.nonce));
            println!("      \"plaintextHex\": \"{}\",", hex_lower(case.plaintext));
            println!("      \"sealedHex\": \"{}\"", hex_lower(&sealed));
            println!("    }}{comma}");
        }
        println!("  ],");

        println!("  \"deviceSeal\": [");
        for (i, case) in device_cases.iter().enumerate() {
            let key = derive_device_seal_key(&case.device_secret);
            let sealed =
                seal_with_nonce(&key, &case.nonce, case.plaintext, case.aad_role.as_bytes());
            let comma = if i + 1 == device_cases.len() { "" } else { "," };
            println!("    {{");
            println!(
                "      \"deviceSecretHex\": \"{}\",",
                hex32(&case.device_secret)
            );
            println!("      \"sealKeyHex\": \"{}\",", hex_lower(&key));
            println!("      \"aadRole\": \"{}\",", case.aad_role);
            println!("      \"nonceHex\": \"{}\",", hex_lower(&case.nonce));
            println!("      \"plaintextHex\": \"{}\",", hex_lower(case.plaintext));
            println!("      \"sealedHex\": \"{}\"", hex_lower(&sealed));
            println!("    }}{comma}");
        }
        println!("  ],");

        println!("  \"meetRoom\": [");
        for (i, case) in room_cases.iter().enumerate() {
            let room = derive_meet_room(&case.identity_secret);
            let comma = if i + 1 == room_cases.len() { "" } else { "," };
            println!("    {{");
            println!(
                "      \"identitySecretHex\": \"{}\",",
                hex32(&case.identity_secret)
            );
            println!("      \"meetRoom\": \"{room}\"");
            println!("    }}{comma}");
        }
        println!("  ]");
        println!("}}");
    }
}
