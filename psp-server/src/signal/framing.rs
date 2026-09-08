use std::collections::HashMap;

use serde_json::{json, Value};

pub const CTL_DIRECT_MAX: usize = 56 * 1024;

pub const CHUNK_SEGMENT: usize = 28 * 1024;

const MAX_CONCURRENT_IDS: usize = 4;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;

pub fn chunk_frame(frame: String, next_id: &mut u64) -> Vec<String> {
    if frame.len() <= CTL_DIRECT_MAX {
        return vec![frame];
    }

    let id = *next_id;
    *next_id += 1;

    let mut bounds = Vec::new();
    let mut start = 0;
    while start < frame.len() {
        let end = chunk_end(&frame, start);
        bounds.push((start, end));
        start = end;
    }

    let parts = bounds.len();
    bounds
        .into_iter()
        .enumerate()
        .map(|(part, (from, to))| {
            json!({
                "type": "chunk",
                "data": { "id": id, "part": part, "parts": parts, "data": &frame[from..to] }
            })
            .to_string()
        })
        .collect()
}

fn chunk_end(text: &str, start: usize) -> usize {
    let end = floor_char_boundary(text, (start + CHUNK_SEGMENT).min(text.len()));
    if end <= start {
        next_char_boundary(text, start)
    } else {
        end
    }
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn next_char_boundary(text: &str, start: usize) -> usize {
    text[start..]
        .chars()
        .next()
        .map_or(text.len(), |c| start + c.len_utf8())
}

struct PartialFrame {
    parts: Vec<Option<String>>,
    received: usize,
    bytes: usize,
}

#[derive(Default)]
pub struct ChunkAssembler {
    pending: HashMap<u64, PartialFrame>,
    total_bytes: usize,
}

#[derive(Debug)]
pub enum AssemblerOutcome {
    NotChunk(Value),
    Pending,
    Complete(String),
    Rejected,
}

struct Chunk {
    id: u64,
    part: usize,
    parts: usize,
    data: String,
}

fn parse_chunk(envelope: &Value) -> Option<Chunk> {
    let data = envelope.get("data")?;
    let id = data.get("id")?.as_u64()?;
    let part = usize::try_from(data.get("part")?.as_u64()?).ok()?;
    let parts = usize::try_from(data.get("parts")?.as_u64()?).ok()?;
    if parts == 0 || part >= parts {
        return None;
    }
    let data = data.get("data")?.as_str()?.to_string();
    Some(Chunk {
        id,
        part,
        parts,
        data,
    })
}

impl ChunkAssembler {
    pub fn accept(&mut self, envelope: Value) -> AssemblerOutcome {
        if envelope.get("type").and_then(Value::as_str) != Some("chunk") {
            return AssemblerOutcome::NotChunk(envelope);
        }
        let Some(chunk) = parse_chunk(&envelope) else {
            return AssemblerOutcome::Rejected;
        };

        if !self.pending.contains_key(&chunk.id) && self.pending.len() >= MAX_CONCURRENT_IDS {
            return AssemblerOutcome::Rejected;
        }

        let entry = self
            .pending
            .entry(chunk.id)
            .or_insert_with(|| PartialFrame {
                parts: vec![None; chunk.parts],
                received: 0,
                bytes: 0,
            });

        if entry.parts.len() != chunk.parts || entry.parts[chunk.part].is_some() {
            self.total_bytes -= entry.bytes;
            self.pending.remove(&chunk.id);
            return AssemblerOutcome::Rejected;
        }

        let added = chunk.data.len();
        if self.total_bytes + added > MAX_TOTAL_BYTES {
            self.total_bytes -= entry.bytes;
            self.pending.remove(&chunk.id);
            return AssemblerOutcome::Rejected;
        }

        entry.received += 1;
        entry.bytes += added;
        self.total_bytes += added;
        entry.parts[chunk.part] = Some(chunk.data);

        if entry.received < entry.parts.len() {
            return AssemblerOutcome::Pending;
        }

        let entry = self.pending.remove(&chunk.id).expect("just inserted above");
        self.total_bytes -= entry.bytes;
        let frame = entry
            .parts
            .into_iter()
            .map(|part| part.expect("received count guarantees every part is filled"))
            .collect();
        AssemblerOutcome::Complete(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_frame_passes_through_unchanged() {
        let mut id = 0;
        let out = chunk_frame(r#"{"type":"pong","data":null}"#.to_string(), &mut id);
        assert_eq!(out, vec![r#"{"type":"pong","data":null}"#.to_string()]);
    }

    #[test]
    fn large_frame_round_trips_through_the_assembler() {
        let payload = "x".repeat(200 * 1024);
        let frame = format!(r#"{{"type":"list_servers","data":"{payload}"}}"#);
        let mut id = 0;
        let pieces = chunk_frame(frame.clone(), &mut id);
        assert!(pieces.len() > 1);
        let mut assembler = ChunkAssembler::default();
        let mut complete = None;
        for piece in &pieces {
            match assembler.accept(serde_json::from_str(piece).unwrap()) {
                AssemblerOutcome::Complete(text) => complete = Some(text),
                AssemblerOutcome::Pending => {}
                other => panic!("unexpected: {other:?}"),
            }
        }
        assert_eq!(complete.unwrap(), frame);
    }

    #[test]
    fn every_chunk_stays_under_the_datachannel_cap_even_when_fully_escaped() {
        let frame = format!(r#"{{"type":"t","data":"{}"}}"#, r#"\""#.repeat(120 * 1024));
        let mut id = 0;
        for piece in chunk_frame(frame, &mut id) {
            assert!(piece.len() <= 64 * 1024, "chunk {} bytes", piece.len());
        }
    }

    #[test]
    fn chunk_boundaries_respect_utf8() {
        let frame = format!(r#"{{"type":"t","data":"{}"}}"#, "🦙".repeat(20 * 1024));
        let mut id = 0;
        let pieces = chunk_frame(frame.clone(), &mut id);
        let mut assembler = ChunkAssembler::default();
        let last = pieces
            .iter()
            .map(|p| assembler.accept(serde_json::from_str(p).unwrap()))
            .last();
        assert!(matches!(last, Some(AssemblerOutcome::Complete(text)) if text == frame));
    }

    #[test]
    fn non_chunk_envelopes_pass_through() {
        let mut assembler = ChunkAssembler::default();
        let value = serde_json::json!({"type":"list_servers"});
        assert!(
            matches!(assembler.accept(value.clone()), AssemblerOutcome::NotChunk(v) if v == value)
        );
    }

    #[test]
    fn too_many_concurrent_ids_are_rejected() {
        let mut assembler = ChunkAssembler::default();
        for id in 0..4u64 {
            let piece =
                serde_json::json!({"type":"chunk","data":{"id":id,"part":0,"parts":2,"data":"a"}});
            assert!(matches!(assembler.accept(piece), AssemblerOutcome::Pending));
        }
        let fifth =
            serde_json::json!({"type":"chunk","data":{"id":9,"part":0,"parts":2,"data":"a"}});
        assert!(matches!(
            assembler.accept(fifth),
            AssemblerOutcome::Rejected
        ));
    }

    #[test]
    fn malformed_chunks_are_rejected_not_panicked() {
        let mut assembler = ChunkAssembler::default();
        for bad in [
            serde_json::json!({"type":"chunk"}),
            serde_json::json!({"type":"chunk","data":{"id":0,"part":5,"parts":2,"data":"a"}}),
            serde_json::json!({"type":"chunk","data":{"id":0,"parts":0,"part":0,"data":"a"}}),
        ] {
            assert!(matches!(assembler.accept(bad), AssemblerOutcome::Rejected));
        }
    }

    #[test]
    fn resending_an_already_received_part_is_rejected_and_frees_its_bytes() {
        let mut assembler = ChunkAssembler::default();
        let first =
            serde_json::json!({"type":"chunk","data":{"id":1,"part":0,"parts":2,"data":"a"}});
        assert!(matches!(assembler.accept(first), AssemblerOutcome::Pending));
        assert_eq!(assembler.total_bytes, 1);

        let resend = serde_json::json!({
            "type":"chunk","data":{"id":1,"part":0,"parts":2,"data":"a".repeat(1_000_000)}
        });
        assert!(matches!(
            assembler.accept(resend),
            AssemblerOutcome::Rejected
        ));
        assert_eq!(
            assembler.total_bytes, 0,
            "the rejected id's state, and its byte accounting, must be dropped"
        );
    }

    #[test]
    fn a_dropped_ids_bytes_return_to_the_budget() {
        let mut assembler = ChunkAssembler::default();
        let almost_full = "x".repeat(MAX_TOTAL_BYTES - 10);
        let first = serde_json::json!({
            "type":"chunk","data":{"id":1,"part":0,"parts":2,"data": almost_full}
        });
        assert!(matches!(assembler.accept(first), AssemblerOutcome::Pending));
        assert_eq!(assembler.total_bytes, MAX_TOTAL_BYTES - 10);

        let second = serde_json::json!({
            "type":"chunk","data":{"id":1,"part":1,"parts":2,"data":"y".repeat(20)}
        });
        assert!(matches!(
            assembler.accept(second),
            AssemblerOutcome::Rejected
        ));
        assert_eq!(
            assembler.total_bytes, 0,
            "the dropped id must return its bytes to the budget"
        );

        let mut id = 5;
        let frame = format!(r#"{{"type":"t","data":"{}"}}"#, "z".repeat(60 * 1024));
        let pieces = chunk_frame(frame.clone(), &mut id);
        let mut complete = None;
        for piece in &pieces {
            if let AssemblerOutcome::Complete(text) =
                assembler.accept(serde_json::from_str(piece).unwrap())
            {
                complete = Some(text);
            }
        }
        assert_eq!(complete, Some(frame));
    }
}
