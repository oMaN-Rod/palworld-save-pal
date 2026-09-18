mod common;

use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Value};

use ps_server::services::mods::uploads::SWEEP_EVERY;

static FAST_SWEEP: std::sync::OnceLock<()> = std::sync::OnceLock::new();

async fn start_server() -> common::TestServer {
    FAST_SWEEP.get_or_init(|| std::env::set_var("PS_UPLOAD_SWEEP_SECS", "1"));
    common::start_test_server().await
}

async fn request(ws: &mut common::WsClient, message_type: &str, data: Value) -> Value {
    common::send_json(ws, json!({ "type": message_type, "data": data })).await;
    let (frame, _) = common::mods_ws::next_of_type(ws, message_type).await;
    frame["data"].clone()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn pseudo_random_bytes(len: usize) -> Vec<u8> {
    let mut state: u32 = 0x2545_f491;
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect()
}

fn write_zip(path: &Path, entries: &[(&str, &[u8], zip::CompressionMethod)]) {
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    for (name, body, method) in entries {
        let options: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(*method);
        writer.start_file(*name, options).unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap();
}

async fn add_target(ws: &mut common::WsClient, root: &Path) -> String {
    let reply = request(
        ws,
        "mod_target_add",
        json!({ "root_path": root.to_string_lossy() }),
    )
    .await;
    reply["target"]["id"].as_str().unwrap().to_string()
}

fn pending_count(server: &common::TestServer) -> usize {
    std::fs::read_dir(server._temp_dir.path().join("downloads").join(".uploads"))
        .unwrap()
        .count()
}

async fn begin(ws: &mut common::WsClient, name: &str, size: usize, sha256: &str) -> String {
    let begun = request(
        ws,
        "mod_upload_begin",
        json!({ "name": name, "size": size, "sha256": sha256 }),
    )
    .await;
    assert!(begun.get("error").is_none(), "{begun:?}");
    begun["upload_id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn an_uploaded_archive_can_be_analyzed() {
    let server = start_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let scratch = tempfile::tempdir().unwrap();
    let zip_path = scratch.path().join("CoolMod-1.0.zip");
    let padding = pseudo_random_bytes(1536 * 1024);
    write_zip(
        &zip_path,
        &[
            (
                "CoolMod/Scripts/main.lua",
                b"print('hi')",
                zip::CompressionMethod::Deflated,
            ),
            (
                "CoolMod/Scripts/pad.bin",
                &padding,
                zip::CompressionMethod::Stored,
            ),
        ],
    );
    let archive = std::fs::read(&zip_path).unwrap();

    let begun = request(
        &mut ws,
        "mod_upload_begin",
        json!({ "name": "CoolMod-1.0.zip", "size": archive.len(), "sha256": sha256_hex(&archive) }),
    )
    .await;
    assert!(begun.get("error").is_none(), "{begun:?}");
    assert_eq!(begun["name"], "CoolMod-1.0.zip");
    let upload_id = begun["upload_id"].as_str().unwrap().to_string();
    let chunk_size = begun["chunk_size"].as_u64().unwrap() as usize;

    let chunks: Vec<&[u8]> = archive.chunks(chunk_size).collect();
    assert!(chunks.len() >= 2, "{} chunks", chunks.len());
    let mut received = 0;
    for (seq, chunk) in chunks.iter().enumerate() {
        let reply = request(
            &mut ws,
            "mod_upload_chunk",
            json!({ "upload_id": upload_id, "seq": seq, "data_b64": encode(chunk) }),
        )
        .await;
        received += chunk.len();
        assert!(reply.get("error").is_none(), "{reply:?}");
        assert_eq!(reply["upload_id"], upload_id.as_str());
        assert_eq!(reply["seq"], seq);
        assert_eq!(reply["received"], received);
    }

    let ended = request(&mut ws, "mod_upload_end", json!({ "upload_id": upload_id })).await;
    assert!(ended.get("error").is_none(), "{ended:?}");
    let path = ended["path"].as_str().unwrap().to_string();
    assert!(
        Path::new(&path).starts_with(server._temp_dir.path().join("downloads")),
        "{path}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), archive);
    assert_eq!(pending_count(&server), 0);

    let analyzed = request(
        &mut ws,
        "mod_analyze",
        json!({ "path": path, "target_id": target_id }),
    )
    .await;
    assert!(analyzed.get("error").is_none(), "{analyzed:?}");
    assert!(
        !analyzed["manifest"]["routes"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{analyzed:?}"
    );
}

#[tokio::test]
async fn an_out_of_order_chunk_is_refused_and_the_upload_continues() {
    let server = start_server().await;
    let mut ws = common::connect(&server).await;
    let upload_id = begin(&mut ws, "a.zip", 6, &sha256_hex(b"abcdef")).await;

    let refused = request(
        &mut ws,
        "mod_upload_chunk",
        json!({ "upload_id": upload_id, "seq": 1, "data_b64": encode(b"abcdef") }),
    )
    .await;
    assert_eq!(
        refused["error"]["code"], "chunk_out_of_order",
        "{refused:?}"
    );
    assert_eq!(refused["error"]["expected_seq"], 0);
    assert_eq!(refused["upload_id"], upload_id.as_str());
    assert_eq!(refused["seq"], 1);

    let accepted = request(
        &mut ws,
        "mod_upload_chunk",
        json!({ "upload_id": upload_id, "seq": 0, "data_b64": encode(b"abcdef") }),
    )
    .await;
    assert!(accepted.get("error").is_none(), "{accepted:?}");
    assert_eq!(accepted["received"], 6);

    let ended = request(&mut ws, "mod_upload_end", json!({ "upload_id": upload_id })).await;
    assert!(ended.get("error").is_none(), "{ended:?}");
    assert_eq!(
        std::fs::read(ended["path"].as_str().unwrap()).unwrap(),
        b"abcdef"
    );
    assert_eq!(pending_count(&server), 0);
}

#[tokio::test]
async fn a_wrong_hash_is_refused_at_end() {
    let server = start_server().await;
    let mut ws = common::connect(&server).await;
    let upload_id = begin(&mut ws, "a.zip", 6, &sha256_hex(b"zzzzzz")).await;
    let accepted = request(
        &mut ws,
        "mod_upload_chunk",
        json!({ "upload_id": upload_id, "seq": 0, "data_b64": encode(b"abcdef") }),
    )
    .await;
    assert!(accepted.get("error").is_none(), "{accepted:?}");

    let ended = request(&mut ws, "mod_upload_end", json!({ "upload_id": upload_id })).await;
    assert_eq!(ended["error"]["code"], "hash_mismatch", "{ended:?}");
    assert_eq!(ended["upload_id"], upload_id.as_str());
    assert_eq!(pending_count(&server), 0);

    let gone = request(
        &mut ws,
        "mod_upload_chunk",
        json!({ "upload_id": upload_id, "seq": 1, "data_b64": encode(b"a") }),
    )
    .await;
    assert_eq!(gone["error"]["code"], "upload_not_found", "{gone:?}");
}

#[tokio::test]
async fn a_disconnected_client_leaves_nothing_behind_after_a_sweep() {
    let server = start_server().await;
    let mut ws = common::connect(&server).await;
    let upload_id = begin(&mut ws, "a.zip", 6, &sha256_hex(b"abcdef")).await;
    let accepted = request(
        &mut ws,
        "mod_upload_chunk",
        json!({ "upload_id": upload_id, "seq": 0, "data_b64": encode(b"abc") }),
    )
    .await;
    assert_eq!(accepted["received"], 3, "{accepted:?}");
    assert_eq!(pending_count(&server), 1);

    ws.close(None).await.unwrap();

    let deadline = Instant::now() + SWEEP_EVERY + Duration::from_secs(5);
    while pending_count(&server) > 0 {
        assert!(
            Instant::now() < deadline,
            "the partial upload outlived its connection"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
