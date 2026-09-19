//! The PST container. `.pstbase` is `zstd(brotli(cbor(payload)))`; `.json` is the same
//! payload as plain JSON, except that some exports wrap byte arrays as `{"~b": "<base64>"}`
//! rather than inlining them. All forms normalize to one `serde_json::Value`, with byte
//! arrays as integer arrays, so nothing downstream branches on the encoding.

use crate::error::CoreError;
use base64::Engine as _;
use serde_json::Value;

/// The lone key of PST's base64-wrapped byte-array form: `{"~b": "<base64>"}`.
const BASE64_BYTES_KEY: &str = "~b";

/// zstd's frame magic.
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

/// The largest payload we will decompress. The corpus's largest legitimate sample is
/// ~92 MB, so this admits real files while refusing a decompression bomb, which is a
/// plausible hostile input for a file users download from strangers.
pub const MAX_DECOMPRESSED_BYTES: usize = 256 * 1024 * 1024;

pub fn is_pstbase(bytes: &[u8]) -> bool {
    bytes.starts_with(&ZSTD_MAGIC)
}

pub fn decode(bytes: &[u8]) -> Result<Value, CoreError> {
    if is_pstbase(bytes) {
        let brotli_bytes = unzstd(bytes)?;
        let cbor_bytes = unbrotli(&brotli_bytes)?;
        decode_cbor(&cbor_bytes)
    } else {
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|e| CoreError::Parse(format!("not a PST blueprint: {e}")))?;
        normalize_json(value)
    }
}

/// Rewrites `{"~b": "<base64>"}` objects into the same integer-array shape the CBOR
/// path produces, recursing everywhere else unchanged. A `~b` key sharing an object
/// with other keys is left alone -- it is not this marker, just a field that happens
/// to be named `~b`.
fn normalize_json(value: Value) -> Result<Value, CoreError> {
    Ok(match value {
        Value::Object(map) => {
            if map.len() == 1 {
                if let Some(Value::String(encoded)) = map.get(BASE64_BYTES_KEY) {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .map_err(|e| CoreError::Parse(format!("invalid ~b base64: {e}")))?;
                    return Ok(Value::Array(
                        bytes.into_iter().map(|b| Value::from(b as u64)).collect(),
                    ));
                }
            }
            let mut object = serde_json::Map::new();
            for (key, value) in map {
                object.insert(key, normalize_json(value)?);
            }
            Value::Object(object)
        }
        Value::Array(items) => {
            Value::Array(items.into_iter().map(normalize_json).collect::<Result<_, _>>()?)
        }
        other => other,
    })
}

/// CBOR carries byte strings natively where JSON carries integer arrays. Normalizing
/// here is what lets one assembler serve both encodings.
pub fn decode_cbor(bytes: &[u8]) -> Result<Value, CoreError> {
    let value: ciborium::Value = ciborium::from_reader(bytes)
        .map_err(|e| CoreError::Parse(format!("pstbase cbor decode failed: {e}")))?;
    normalize(value)
}

fn normalize(value: ciborium::Value) -> Result<Value, CoreError> {
    Ok(match value {
        ciborium::Value::Bytes(bytes) => {
            Value::Array(bytes.into_iter().map(|b| Value::from(b as u64)).collect())
        }
        ciborium::Value::Array(items) => {
            Value::Array(items.into_iter().map(normalize).collect::<Result<_, _>>()?)
        }
        ciborium::Value::Map(entries) => {
            let mut object = serde_json::Map::new();
            for (key, value) in entries {
                let key = key
                    .into_text()
                    .map_err(|_| CoreError::Parse("pstbase map key is not a string".into()))?;
                object.insert(key, normalize(value)?);
            }
            Value::Object(object)
        }
        ciborium::Value::Text(text) => Value::String(text),
        ciborium::Value::Bool(flag) => Value::Bool(flag),
        ciborium::Value::Null => Value::Null,
        ciborium::Value::Integer(n) => {
            let n: i128 = n.into();
            serde_json::Number::from_i128(n).map(Value::Number).unwrap_or(Value::Null)
        }
        ciborium::Value::Float(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        other => {
            return Err(CoreError::Parse(format!("unsupported cbor value: {other:?}")))
        }
    })
}

fn unzstd(bytes: &[u8]) -> Result<Vec<u8>, CoreError> {
    let decoder = ruzstd::StreamingDecoder::new(bytes)
        .map_err(|e| CoreError::Parse(format!("pstbase zstd decode failed: {e}")))?;
    read_capped(decoder, "zstd")
}

fn unbrotli(bytes: &[u8]) -> Result<Vec<u8>, CoreError> {
    let decoder = brotli::Decompressor::new(bytes, 4096);
    read_capped(decoder, "brotli")
}

/// Reads at most `MAX_DECOMPRESSED_BYTES`, refusing rather than allocating beyond it.
fn read_capped<R: std::io::Read>(reader: R, stage: &str) -> Result<Vec<u8>, CoreError> {
    let mut out = Vec::new();
    let mut limited = reader.take(MAX_DECOMPRESSED_BYTES as u64 + 1);
    std::io::Read::read_to_end(&mut limited, &mut out)
        .map_err(|e| CoreError::Parse(format!("pstbase {stage} decode failed: {e}")))?;
    if out.len() > MAX_DECOMPRESSED_BYTES {
        return Err(CoreError::Parse(format!(
            "pstbase {stage} payload is too large: over {MAX_DECOMPRESSED_BYTES} bytes"
        )));
    }
    Ok(out)
}

#[doc(hidden)]
pub mod test_support {
    /// A zstd frame whose content size header claims `size` bytes, for exercising the cap.
    pub fn zstd_frame_claiming(size: usize) -> Vec<u8> {
        let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0x00];
        frame.extend_from_slice(&(size as u64).to_le_bytes());
        frame
    }
}
