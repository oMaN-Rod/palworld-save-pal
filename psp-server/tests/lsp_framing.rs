use psp_server::services::lsp_process::{encode_frame, FrameOutcome, FrameReader};
use serde_json::json;

#[test]
fn a_frame_carries_a_byte_length_header_and_a_blank_line() {
    let bytes = encode_frame(&json!({ "jsonrpc": "2.0", "id": 1 }));
    let text = String::from_utf8(bytes).expect("utf8");
    let (header, body) = text.split_once("\r\n\r\n").expect("a header separator");
    assert_eq!(header, format!("Content-Length: {}", body.len()));
}

#[test]
fn the_length_is_bytes_not_characters() {
    let bytes = encode_frame(&json!({ "text": "héllo — ünïcode" }));
    let text = String::from_utf8(bytes).expect("utf8");
    let (header, body) = text.split_once("\r\n\r\n").expect("a header separator");
    let declared: usize = header
        .trim_start_matches("Content-Length: ")
        .parse()
        .expect("a number");
    assert_eq!(declared, body.as_bytes().len());
    assert_ne!(
        declared,
        body.chars().count(),
        "the fixture must contain multibyte text"
    );
}

#[test]
fn a_reader_yields_two_frames_delivered_in_one_chunk() {
    let mut reader = FrameReader::new();
    let mut buffer = encode_frame(&json!({ "id": 1 }));
    buffer.extend(encode_frame(&json!({ "id": 2 })));
    reader.push(&buffer);
    assert_eq!(reader.next_frame().expect("frame 1")["id"], 1);
    assert_eq!(reader.next_frame().expect("frame 2")["id"], 2);
    assert!(reader.next_frame().is_none());
}

#[test]
fn a_reader_waits_for_a_frame_split_across_chunks() {
    let mut reader = FrameReader::new();
    let bytes = encode_frame(&json!({ "id": 7, "padding": "aaaaaaaaaaaaaaaaaaaa" }));
    let (head, tail) = bytes.split_at(12);
    reader.push(head);
    assert!(
        reader.next_frame().is_none(),
        "a partial frame must not be yielded"
    );
    reader.push(tail);
    assert_eq!(reader.next_frame().expect("the completed frame")["id"], 7);
}

#[test]
fn a_reader_survives_an_unparsable_body_without_losing_the_next_frame() {
    let mut reader = FrameReader::new();
    let mut buffer = b"Content-Length: 3\r\n\r\n{{{".to_vec();
    buffer.extend(encode_frame(&json!({ "id": 9 })));
    reader.push(&buffer);
    assert!(reader.next_frame().is_none(), "the bad body is dropped");
    assert_eq!(reader.next_frame().expect("the good frame")["id"], 9);
}

/// Pins the production consumer's shape: pull by `FrameOutcome` and keep
/// going on `Dropped`, stopping only on `Incomplete`. A single `push`
/// covering a bad frame followed by a good one must yield the good frame
/// without a second push — the bug a plain `while let Some(...) = next_frame()`
/// loop has, since a dropped frame looks identical to "no more data yet".
#[test]
fn a_consumer_pulling_by_outcome_reaches_the_good_frame_after_one_push() {
    let mut reader = FrameReader::new();
    let mut buffer = b"Content-Length: 3\r\n\r\n{{{".to_vec();
    buffer.extend(encode_frame(&json!({ "id": 9 })));
    reader.push(&buffer);

    let mut frames = Vec::new();
    loop {
        match reader.next_outcome() {
            FrameOutcome::Frame(value) => frames.push(value),
            FrameOutcome::Dropped => continue,
            FrameOutcome::Incomplete => break,
            FrameOutcome::Fatal(reason) => panic!("unexpected fatal outcome: {reason}"),
        }
    }

    assert_eq!(frames.len(), 1, "exactly the good frame must be yielded");
    assert_eq!(frames[0]["id"], 9);
}

#[test]
fn a_hostile_content_length_is_rejected_without_overflow_or_panic() {
    let mut reader = FrameReader::new();
    reader.push(b"Content-Length: 18446744073709551615\r\n\r\n");
    match reader.next_outcome() {
        FrameOutcome::Fatal(_) => {}
        other => panic!("expected a Fatal outcome, got {other:?}"),
    }
}

#[test]
fn a_declared_length_far_past_any_real_message_is_rejected() {
    let mut reader = FrameReader::new();
    reader.push(format!("Content-Length: {}\r\n\r\n", 500_000_000).as_bytes());
    match reader.next_outcome() {
        FrameOutcome::Fatal(_) => {}
        other => panic!("expected a Fatal outcome, got {other:?}"),
    }
}

#[test]
fn a_header_that_never_arrives_is_capped_instead_of_growing_forever() {
    let mut reader = FrameReader::new();
    reader.push(&vec![b'x'; 200_000]);
    match reader.next_outcome() {
        FrameOutcome::Fatal(_) => {}
        other => panic!("expected a Fatal outcome, got {other:?}"),
    }
}

/// The cap must hold even when a single `push` delivers an oversized header
/// together with its separator in one shot — not only when the separator
/// never shows up at all.
#[test]
fn an_oversized_header_is_capped_even_when_its_separator_arrives_in_the_same_push() {
    let mut reader = FrameReader::new();
    let mut buffer = vec![b'x'; 200_000];
    buffer.extend_from_slice(b"\r\n\r\n");
    reader.push(&buffer);
    match reader.next_outcome() {
        FrameOutcome::Fatal(_) => {}
        other => panic!("expected a Fatal outcome, got {other:?}"),
    }
}
