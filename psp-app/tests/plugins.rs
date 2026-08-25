use base64::Engine as _;

use psp_app::dispatcher::HandlerCtx;
use psp_app::handlers::plugins::*;
use psp_app::test_support::TestContext;
use psp_db::plugins::NewPlugin;

fn ctx<'a>(test: &'a mut TestContext) -> HandlerCtx<'a> {
    HandlerCtx {
        session: &mut test.session,
        app: &test.app,
        emitter: &test.emitter,
        blueprints: &mut test.blueprints,
        attachment: None,
    }
}

fn minimal_save() -> psp_core::session::SaveSession {
    use psp_core::ue::*;

    let save = Save {
        header: Header {
            magic: 0,
            save_game_version: 0,
            package_version: PackageVersion { ue4: 0, ue5: None },
            engine_version_major: 0,
            engine_version_minor: 0,
            engine_version_patch: 0,
            engine_version_build: 0,
            engine_version: String::new(),
            custom_version: None,
        },
        schemas: PropertySchemas::default(),
        root: Root { save_game_type: String::new(), properties: Properties::default() },
        extra: Vec::new(),
    };
    psp_core::session::SaveSession::new_for_tests(psp_core::session::SaveKind::InMemory, save)
}

fn manifest_json(id: &str, capabilities: &[&str], command_id: &str, api_version: u32) -> String {
    serde_json::json!({
        "id": id,
        "api_version": api_version,
        "name": id,
        "version": "1.0.0",
        "entry": "main.lua",
        "capabilities": capabilities,
        "commands": [{"id": command_id, "title": command_id}],
    })
    .to_string()
}

async fn seed_row(
    test: &TestContext,
    id: &str,
    capabilities: &[&str],
    script: &str,
    granted: &[&str],
    bundled: bool,
) -> psp_db::plugins::PluginRow {
    let manifest = manifest_json(id, capabilities, "run", 1);
    let sources = serde_json::json!({ "main.lua": script }).to_string();
    let granted_json = serde_json::to_string(granted).unwrap();
    psp_db::plugins::upsert(
        &*test.app.driver,
        &NewPlugin {
            id,
            manifest: &manifest,
            sources: &sources,
            granted_capabilities: &granted_json,
            bundled,
        },
    )
    .await
    .unwrap()
}

fn install_data(filename: &str, content: &[u8]) -> InstallPluginData {
    InstallPluginData {
        filename: filename.to_string(),
        content: base64::engine::general_purpose::STANDARD.encode(content),
    }
}

fn zip_bytes(entries: &[(&str, &str)]) -> Vec<u8> {
    use std::io::Write;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default();
        for (name, contents) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }
    cursor.into_inner()
}

#[tokio::test]
async fn list_plugins_on_a_fresh_database_returns_the_bundled_set() {
    let mut test = TestContext::new(|_| {}).await;
    seed_bundled_plugins(&test.app).await.unwrap();

    handle_list_plugins(&mut ctx(&mut test)).await.unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "list_plugins");
    let list = frame["data"].as_array().unwrap();
    assert_eq!(list.len(), psp_app::plugin_registry::BUNDLED.len());
}

#[tokio::test]
async fn get_plugin_returns_the_manifest_and_sources() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "sample", &["log"], "function run() end", &["log"], false).await;

    handle_get_plugin(PluginIdData { id: "sample".to_string() }, &mut ctx(&mut test))
        .await
        .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "get_plugin");
    assert_eq!(frame["data"]["id"], "sample");
    assert_eq!(frame["data"]["manifest"]["id"], "sample");
    assert_eq!(frame["data"]["sources"]["main.lua"], "function run() end");
    assert_eq!(frame["data"]["granted_capabilities"], serde_json::json!(["log"]));
}

#[tokio::test]
async fn get_plugin_for_an_unknown_id_emits_an_error_frame_not_a_panic() {
    let mut test = TestContext::new(|_| {}).await;
    handle_get_plugin(PluginIdData { id: "does-not-exist".to_string() }, &mut ctx(&mut test))
        .await
        .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "error");
}

#[tokio::test]
async fn install_plugin_accepts_a_bare_lua_file_and_synthesises_a_manifest() {
    let mut test = TestContext::new(|_| {}).await;
    let data = install_data("MyCoolPlugin.lua", b"function main()\n  return 'hi'\nend\n");

    handle_install_plugin(data, &mut ctx(&mut test)).await.unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "install_plugin");
    let id = frame["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(id, "mycoolplugin");

    let row = psp_db::plugins::get(&*test.app.driver, &id).await.unwrap().unwrap();
    assert!(!row.bundled);
    let sources: serde_json::Value = serde_json::from_str(&row.sources).unwrap();
    assert_eq!(sources["main.lua"], "function main()\n  return 'hi'\nend\n");
}

#[tokio::test]
async fn install_plugin_accepts_a_zip_containing_a_manifest_and_sources() {
    let mut test = TestContext::new(|_| {}).await;
    let manifest = manifest_json("zipped", &[], "run", 1);
    let bytes = zip_bytes(&[
        ("manifest.json", &manifest),
        ("main.lua", "function run() return 'ok' end"),
    ]);
    let data = install_data("plugin.zip", &bytes);

    handle_install_plugin(data, &mut ctx(&mut test)).await.unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "install_plugin");
    assert_eq!(frame["data"]["id"], "zipped");

    let row = psp_db::plugins::get(&*test.app.driver, "zipped").await.unwrap().unwrap();
    let sources: serde_json::Value = serde_json::from_str(&row.sources).unwrap();
    assert_eq!(sources["main.lua"], "function run() return 'ok' end");
}

#[tokio::test]
async fn install_plugin_accepts_a_manifest_requesting_save_raw() {
    let mut test = TestContext::new(|_| {}).await;
    let manifest = manifest_json("raw-plugin", &["save.raw"], "run", 1);
    let bytes = zip_bytes(&[("manifest.json", &manifest), ("main.lua", "function run() end")]);
    let data = install_data("plugin.zip", &bytes);

    handle_install_plugin(data, &mut ctx(&mut test)).await.unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "install_plugin");
    assert_eq!(frame["data"]["id"], "raw-plugin");

    assert!(psp_db::plugins::get(&*test.app.driver, "raw-plugin").await.unwrap().is_some());
}

#[tokio::test]
async fn install_plugin_rejects_an_unsupported_api_version_with_both_numbers() {
    let mut test = TestContext::new(|_| {}).await;
    let manifest = manifest_json("future-plugin", &[], "run", 2);
    let bytes = zip_bytes(&[("manifest.json", &manifest), ("main.lua", "function run() end")]);
    let data = install_data("plugin.zip", &bytes);

    handle_install_plugin(data, &mut ctx(&mut test)).await.unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "error");
    let message = frame["data"].as_str().unwrap();
    assert!(message.contains('1'), "message was: {message}");
    assert!(message.contains('2'), "message was: {message}");
}

#[tokio::test]
async fn install_plugin_rejects_a_zip_entry_whose_path_escapes_the_archive() {
    let manifest = manifest_json("escaper", &[], "run", 1);

    let mut test = TestContext::new(|_| {}).await;
    let bytes = zip_bytes(&[
        ("manifest.json", &manifest),
        ("../../../etc/passwd", "function run() end"),
    ]);
    handle_install_plugin(install_data("plugin.zip", &bytes), &mut ctx(&mut test))
        .await
        .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "error");
    assert!(psp_db::plugins::get(&*test.app.driver, "escaper").await.unwrap().is_none());

    let mut test2 = TestContext::new(|_| {}).await;
    let bytes2 = zip_bytes(&[
        ("manifest.json", &manifest),
        ("C:\\Windows\\x.lua", "function run() end"),
    ]);
    handle_install_plugin(install_data("plugin.zip", &bytes2), &mut ctx(&mut test2))
        .await
        .unwrap();
    let frame2 = test2.next_frame_json();
    assert_eq!(frame2["type"], "error");
    assert!(psp_db::plugins::get(&*test2.app.driver, "escaper").await.unwrap().is_none());

    // A fullwidth solidus and a one-dot-leader both pass a naive ASCII-only traversal check.
    let mut test3 = TestContext::new(|_| {}).await;
    let bytes3 = zip_bytes(&[
        ("manifest.json", &manifest),
        ("main\u{ff0f}lua", "function run() end"),
    ]);
    handle_install_plugin(install_data("plugin.zip", &bytes3), &mut ctx(&mut test3))
        .await
        .unwrap();
    let frame3 = test3.next_frame_json();
    assert_eq!(frame3["type"], "error");
    assert!(psp_db::plugins::get(&*test3.app.driver, "escaper").await.unwrap().is_none());

    let mut test4 = TestContext::new(|_| {}).await;
    let bytes4 = zip_bytes(&[
        ("manifest.json", &manifest),
        ("main\u{2024}\u{2024}lua", "function run() end"),
    ]);
    handle_install_plugin(install_data("plugin.zip", &bytes4), &mut ctx(&mut test4))
        .await
        .unwrap();
    let frame4 = test4.next_frame_json();
    assert_eq!(frame4["type"], "error");
    assert!(psp_db::plugins::get(&*test4.app.driver, "escaper").await.unwrap().is_none());
}

#[tokio::test]
async fn uninstall_plugin_removes_a_user_plugin() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "removable", &[], "function run() end", &[], false).await;

    handle_uninstall_plugin(PluginIdData { id: "removable".to_string() }, &mut ctx(&mut test))
        .await
        .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "uninstall_plugin");
    assert!(psp_db::plugins::get(&*test.app.driver, "removable").await.unwrap().is_none());
}

#[tokio::test]
async fn uninstall_plugin_refuses_to_remove_a_bundled_plugin() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "core-tool", &[], "function run() end", &[], true).await;

    handle_uninstall_plugin(PluginIdData { id: "core-tool".to_string() }, &mut ctx(&mut test))
        .await
        .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "error");
    assert!(psp_db::plugins::get(&*test.app.driver, "core-tool").await.unwrap().is_some());
}

#[tokio::test]
async fn set_plugin_enabled_toggles_and_answers_with_the_refreshed_list() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "togglable", &[], "function run() end", &[], false).await;

    handle_set_plugin_enabled(
        SetPluginEnabledData { id: "togglable".to_string(), enabled: false },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "list_plugins");
    let list = frame["data"].as_array().unwrap();
    let entry = list.iter().find(|p| p["id"] == "togglable").unwrap();
    assert_eq!(entry["enabled"], false);

    let row = psp_db::plugins::get(&*test.app.driver, "togglable").await.unwrap().unwrap();
    assert!(!row.enabled);
}

#[tokio::test]
async fn run_plugin_command_without_a_loaded_save_answers_with_an_error_result() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "runner", &[], "function run() return 'ok' end", &[], false).await;

    handle_run_plugin_command(
        RunPluginCommandData {
            plugin_id: "runner".to_string(),
            command_id: "run".to_string(),
            args: serde_json::json!({}),
            dry_run: false,
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "plugin_run_result");
    assert_eq!(frame["data"]["status"], "error");
}

#[tokio::test]
async fn run_plugin_command_on_a_disabled_plugin_is_refused() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "disabled-plugin", &[], "function run() end", &[], false).await;
    psp_db::plugins::set_enabled(&*test.app.driver, "disabled-plugin", false).await.unwrap();
    test.session.save = Some(minimal_save());

    handle_run_plugin_command(
        RunPluginCommandData {
            plugin_id: "disabled-plugin".to_string(),
            command_id: "run".to_string(),
            args: serde_json::json!({}),
            dry_run: false,
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "plugin_run_result");
    assert_eq!(frame["data"]["status"], "error");
}

#[tokio::test]
async fn run_plugin_command_emits_a_plugin_run_result_frame_with_status_and_counts() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(
        &test,
        "counter",
        &[],
        "function run() return { summary = 'done', counts = { widgets = 3 } } end",
        &[],
        false,
    )
    .await;
    test.session.save = Some(minimal_save());

    handle_run_plugin_command(
        RunPluginCommandData {
            plugin_id: "counter".to_string(),
            command_id: "run".to_string(),
            args: serde_json::json!({}),
            dry_run: false,
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "plugin_run_result");
    assert_eq!(frame["data"]["status"], "ok");
    assert_eq!(frame["data"]["counts"]["widgets"], 3);
    assert!(frame["data"]["run_id"].is_string());
}

#[tokio::test]
async fn run_plugin_command_with_dry_run_reports_counts_and_changes_nothing() {
    let script = "function run() \
           local applied = 0 \
           if not ctx.dry_run then applied = 1 end \
           return { summary = 'previewed', counts = { applied = applied } } \
         end";

    let mut real_test = TestContext::new(|_| {}).await;
    seed_row(&real_test, "dry-runner", &[], script, &[], false).await;
    real_test.session.save = Some(minimal_save());
    handle_run_plugin_command(
        RunPluginCommandData {
            plugin_id: "dry-runner".to_string(),
            command_id: "run".to_string(),
            args: serde_json::json!({}),
            dry_run: false,
        },
        &mut ctx(&mut real_test),
    )
    .await
    .unwrap();
    let real_frame = real_test.next_frame_json();
    assert_eq!(real_frame["data"]["status"], "ok");
    assert_eq!(real_frame["data"]["counts"]["applied"], 1);

    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "dry-runner", &[], script, &[], false).await;
    test.session.save = Some(minimal_save());

    handle_run_plugin_command(
        RunPluginCommandData {
            plugin_id: "dry-runner".to_string(),
            command_id: "run".to_string(),
            args: serde_json::json!({}),
            dry_run: true,
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["data"]["status"], "ok");
    assert_eq!(frame["data"]["counts"]["applied"], 0);
}

#[tokio::test]
async fn run_plugin_command_only_grants_capabilities_the_row_records() {
    let mut granted_test = TestContext::new(|_| {}).await;
    seed_row(
        &granted_test,
        "revoked",
        &["log"],
        "function run() log.info('hi') end",
        &["log"],
        false,
    )
    .await;
    granted_test.session.save = Some(minimal_save());
    handle_run_plugin_command(
        RunPluginCommandData {
            plugin_id: "revoked".to_string(),
            command_id: "run".to_string(),
            args: serde_json::json!({}),
            dry_run: false,
        },
        &mut ctx(&mut granted_test),
    )
    .await
    .unwrap();
    let granted_frame = granted_test.next_frame_json();
    assert_eq!(granted_frame["data"]["status"], "ok");

    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "revoked", &["log"], "function run() log.info('hi') end", &[], false).await;
    test.session.save = Some(minimal_save());

    handle_run_plugin_command(
        RunPluginCommandData {
            plugin_id: "revoked".to_string(),
            command_id: "run".to_string(),
            args: serde_json::json!({}),
            dry_run: false,
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["data"]["status"], "error");
}

#[tokio::test]
async fn cancel_plugin_run_for_an_unknown_run_id_is_a_no_op_not_an_error() {
    let mut test = TestContext::new(|_| {}).await;
    handle_cancel_plugin_run(
        CancelPluginRunData { run_id: uuid::Uuid::new_v4() },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    test.assert_no_more_frames();
}

#[tokio::test]
async fn a_plugins_storage_writes_are_persisted_after_a_successful_run() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(
        &test,
        "storer",
        &["storage"],
        "function run() storage.set('key', 'value') return 'ok' end",
        &["storage"],
        false,
    )
    .await;
    test.session.save = Some(minimal_save());

    handle_run_plugin_command(
        RunPluginCommandData {
            plugin_id: "storer".to_string(),
            command_id: "run".to_string(),
            args: serde_json::json!({}),
            dry_run: false,
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["data"]["status"], "ok");

    let stored = psp_db::plugins::storage_get_all(&*test.app.driver, "storer").await.unwrap();
    assert_eq!(stored.get("key"), Some(&"value".to_string()));
}

#[tokio::test]
async fn a_plugins_storage_writes_are_discarded_after_a_failed_run() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(
        &test,
        "failer",
        &["storage"],
        "function run() storage.set('key', 'value') error('boom') end",
        &["storage"],
        false,
    )
    .await;
    test.session.save = Some(minimal_save());

    handle_run_plugin_command(
        RunPluginCommandData {
            plugin_id: "failer".to_string(),
            command_id: "run".to_string(),
            args: serde_json::json!({}),
            dry_run: false,
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["data"]["status"], "error");

    let stored = psp_db::plugins::storage_get_all(&*test.app.driver, "failer").await.unwrap();
    assert!(stored.get("key").is_none());
}
/// A minimal hand-built STORED-method zip: `zip::ZipWriter` refuses to write two entries with the same name, but a hand-crafted archive is not obliged to respect that.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn hand_built_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut local = Vec::new();
    let mut central = Vec::new();
    let mut offsets = Vec::new();

    for (name, data) in entries {
        offsets.push(local.len() as u32);
        let name_bytes = name.as_bytes();
        let crc = crc32(data);

        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name_bytes);
        local.extend_from_slice(data);
    }

    for ((name, data), offset) in entries.iter().zip(offsets.iter()) {
        let name_bytes = name.as_bytes();
        let crc = crc32(data);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }

    let central_offset = local.len() as u32;
    let central_size = central.len() as u32;
    let count = entries.len() as u16;

    let mut out = local;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[tokio::test]
async fn install_plugin_refuses_to_overwrite_a_bundled_plugin() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(
        &test,
        "core-tool",
        &[],
        "function run() return 'legit' end",
        &[],
        true,
    )
    .await;

    let manifest = manifest_json("core-tool", &[], "run", 1);
    let bytes = zip_bytes(&[
        ("manifest.json", &manifest),
        ("main.lua", "function run() return 'pwned' end"),
    ]);
    handle_install_plugin(install_data("plugin.zip", &bytes), &mut ctx(&mut test))
        .await
        .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "error");

    let row = psp_db::plugins::get(&*test.app.driver, "core-tool").await.unwrap().unwrap();
    assert!(row.bundled, "the row must still be marked bundled");
    let sources: serde_json::Value = serde_json::from_str(&row.sources).unwrap();
    assert_eq!(sources["main.lua"], "function run() return 'legit' end");
}

#[tokio::test]
async fn install_plugin_rejects_a_zip_entry_name_that_is_only_dots() {
    let manifest = manifest_json("dotty", &[], "run", 1);
    let mut test = TestContext::new(|_| {}).await;
    let bytes = zip_bytes(&[("manifest.json", &manifest), (".", "function run() end")]);
    handle_install_plugin(install_data("plugin.zip", &bytes), &mut ctx(&mut test))
        .await
        .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "error");
    assert!(psp_db::plugins::get(&*test.app.driver, "dotty").await.unwrap().is_none());
}

#[tokio::test]
async fn install_plugin_rejects_a_zip_with_duplicate_entry_names() {
    let manifest_a = manifest_json("dup-plugin", &[], "run", 1);
    let manifest_b = manifest_json("dup-plugin", &["save.raw"], "run", 1);

    let control_bytes = hand_built_zip(&[
        ("manifest.json", manifest_a.as_bytes()),
        ("main.lua", b"function run() end"),
    ]);
    let mut control_test = TestContext::new(|_| {}).await;
    handle_install_plugin(install_data("plugin.zip", &control_bytes), &mut ctx(&mut control_test))
        .await
        .unwrap();
    let control_frame = control_test.next_frame_json();
    assert_eq!(control_frame["type"], "install_plugin");

    // zip::ZipArchive keys its central directory by name, so two entries sharing "manifest.json" collapse to one, keeping the LATER value.
    let bytes = hand_built_zip(&[
        ("manifest.json", manifest_a.as_bytes()),
        ("manifest.json", manifest_b.as_bytes()),
    ]);
    let mut probe = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).unwrap();
    assert_eq!(probe.len(), 1, "the zip crate itself collapses same-name entries");
    let mut collapsed = String::new();
    std::io::Read::read_to_string(&mut probe.by_index(0).unwrap(), &mut collapsed).unwrap();
    assert_eq!(collapsed, manifest_b, "the LATER entry is what survives the collapse");

    let mut test = TestContext::new(|_| {}).await;
    handle_install_plugin(install_data("plugin.zip", &bytes), &mut ctx(&mut test))
        .await
        .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "error");
    assert!(psp_db::plugins::get(&*test.app.driver, "dup-plugin").await.unwrap().is_none());
}

#[tokio::test]
async fn install_plugin_rejects_a_directory_entry_with_an_unsafe_name() {
    let manifest = manifest_json("dir-escaper", &[], "run", 1);
    let bytes = hand_built_zip(&[
        ("manifest.json", manifest.as_bytes()),
        ("main.lua", b"function run() end"),
        ("../escape/", b""),
    ]);
    let mut test = TestContext::new(|_| {}).await;
    handle_install_plugin(install_data("plugin.zip", &bytes), &mut ctx(&mut test))
        .await
        .unwrap();
    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "error");
    assert!(psp_db::plugins::get(&*test.app.driver, "dir-escaper").await.unwrap().is_none());
}

#[tokio::test]
async fn check_plugin_syntax_accepts_a_well_formed_chunk() {
    let mut test = TestContext::new(|_| {}).await;
    handle_check_plugin_syntax(
        CheckPluginSyntaxData { source: "function run() return 1 end".to_string() },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "check_plugin_syntax");
    assert_eq!(frame["data"]["error"], serde_json::Value::Null);
}

#[tokio::test]
async fn check_plugin_syntax_reports_the_line_of_a_parse_error() {
    let mut test = TestContext::new(|_| {}).await;
    handle_check_plugin_syntax(
        CheckPluginSyntaxData { source: "local a = 1\nlocal b = = 2\n".to_string() },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "check_plugin_syntax");
    assert_eq!(frame["data"]["error"]["line"], 2);
    assert!(frame["data"]["error"]["message"].as_str().unwrap().contains("unexpected symbol"));
}

#[tokio::test]
async fn check_plugin_manifest_accepts_a_valid_manifest() {
    let mut test = TestContext::new(|_| {}).await;
    handle_check_plugin_manifest(
        CheckPluginManifestData { manifest: manifest_json("ok.plugin", &["log"], "run", 1) },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "check_plugin_manifest");
    assert_eq!(frame["data"]["error"], serde_json::Value::Null);
}

#[tokio::test]
async fn check_plugin_manifest_reports_a_parse_failure_as_text() {
    let mut test = TestContext::new(|_| {}).await;
    handle_check_plugin_manifest(
        CheckPluginManifestData { manifest: manifest_json("ok.plugin", &["log"], "run", 99) },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "check_plugin_manifest");
    assert!(frame["data"]["error"].as_str().unwrap().contains("99"));
}

#[tokio::test]
async fn check_plugin_manifest_accepts_save_raw_from_any_plugin() {
    let mut test = TestContext::new(|_| {}).await;

    handle_check_plugin_manifest(
        CheckPluginManifestData { manifest: manifest_json("user.one", &["save.raw"], "run", 1) },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(
        frame["data"]["error"],
        serde_json::Value::Null,
        "save.raw is available to every plugin"
    );
    test.assert_no_more_frames();
}

#[tokio::test]
async fn get_api_definition_returns_the_generated_definition() {
    let mut test = TestContext::new(|_| {}).await;
    handle_get_api_definition(&mut ctx(&mut test)).await.unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "get_api_definition");
    let globals = frame["data"]["globals"].as_array().unwrap();
    let names: Vec<&str> = globals.iter().map(|g| g["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"save"), "got globals {names:?}");
    assert!(names.contains(&"ctx"), "got globals {names:?}");
    assert!(!frame["data"]["handles"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn the_api_definition_frame_matches_the_library_value() {
    let mut test = TestContext::new(|_| {}).await;
    handle_get_api_definition(&mut ctx(&mut test)).await.unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["data"], serde_json::to_value(psp_plugin::api_definition()).unwrap());
}

#[tokio::test]
async fn create_plugin_writes_a_runnable_scaffold() {
    let mut test = TestContext::new(|_| {}).await;
    handle_create_plugin(
        CreatePluginData { id: "my.first".to_string(), name: "My First".to_string() },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "create_plugin");
    assert_eq!(frame["data"]["id"], "my.first");
    assert_eq!(frame["data"]["name"], "My First");
    assert!(frame["data"]["error"].is_null());

    let row = psp_db::plugins::get(&*test.app.driver, "my.first").await.unwrap().unwrap();
    assert!(!row.bundled);
    assert!(row.enabled);

    let manifest = psp_plugin::manifest::Manifest::parse(&row.manifest)
        .expect("the scaffold manifest must parse");
    assert_eq!(manifest.entry, "main.lua");
    assert_eq!(manifest.commands.len(), 1);

    let sources: serde_json::Value = serde_json::from_str(&row.sources).unwrap();
    let entry = sources["main.lua"].as_str().expect("the scaffold has a main.lua");
    assert!(
        psp_plugin::syntax::check(entry).is_none(),
        "the scaffold source must parse: {entry}"
    );
    assert!(
        entry.contains(&format!("function {}(", manifest.commands[0].id)),
        "the scaffold must define its declared command: {entry}"
    );

    let granted: Vec<String> = serde_json::from_str(&row.granted_capabilities).unwrap();
    let requested: Vec<String> = manifest
        .capabilities
        .iter()
        .map(|c| serde_json::to_value(c).unwrap().as_str().unwrap().to_string())
        .collect();
    assert_eq!(granted, requested);
}

#[tokio::test]
async fn create_plugin_refuses_an_id_that_already_exists() {
    let mut test = TestContext::new(|_| {}).await;
    let before = seed_row(&test, "taken.id", &["log"], "function run() end", &["log"], false).await;

    handle_create_plugin(
        CreatePluginData { id: "taken.id".to_string(), name: "Taken".to_string() },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "create_plugin");
    assert!(!frame["data"]["error"].is_null());

    let after = psp_db::plugins::get(&*test.app.driver, "taken.id").await.unwrap().unwrap();
    assert_eq!(after.manifest, before.manifest);
    assert_eq!(after.sources, before.sources);
    assert_eq!(after.granted_capabilities, before.granted_capabilities);
}

#[tokio::test]
async fn create_plugin_refuses_an_id_the_manifest_grammar_rejects() {
    let mut test = TestContext::new(|_| {}).await;
    handle_create_plugin(
        CreatePluginData { id: "not a valid id!".to_string(), name: "Bad".to_string() },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "create_plugin");
    assert!(!frame["data"]["error"].is_null());
    assert!(psp_db::plugins::get(&*test.app.driver, "not a valid id!").await.unwrap().is_none());
}

#[tokio::test]
async fn create_plugin_escapes_a_display_name_containing_a_quote() {
    let mut test = TestContext::new(|_| {}).await;
    handle_create_plugin(
        CreatePluginData {
            id: "quoted.name".to_string(),
            name: "Bob's \"Great\" Plugin".to_string(),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let _ = test.next_frame_json();

    let row = psp_db::plugins::get(&*test.app.driver, "quoted.name").await.unwrap().unwrap();
    let sources: serde_json::Value = serde_json::from_str(&row.sources).unwrap();
    let entry = sources["main.lua"].as_str().expect("the scaffold has a main.lua");
    assert!(
        psp_plugin::syntax::check(entry).is_none(),
        "the scaffold source must parse: {entry}"
    );
}

#[tokio::test]
async fn save_plugin_source_replaces_one_file_and_leaves_the_others() {
    let mut test = TestContext::new(|_| {}).await;
    let row = seed_row(&test, "user.one", &["log"], "function run() end", &["log"], false).await;
    let mut sources: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&row.sources).unwrap();
    sources.insert("helper.lua".to_string(), "return 1".to_string());
    psp_db::plugins::set_sources(
        &*test.app.driver,
        "user.one",
        &serde_json::to_string(&sources).unwrap(),
    )
    .await
    .unwrap();

    handle_save_plugin_source(
        SavePluginSourceData {
            id: "user.one".to_string(),
            path: "main.lua".to_string(),
            source: "function run() return 2 end".to_string(),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "save_plugin_source");
    assert_eq!(frame["data"]["id"], "user.one");
    assert_eq!(frame["data"]["path"], "main.lua");

    let row = psp_db::plugins::get(&*test.app.driver, "user.one").await.unwrap().unwrap();
    let stored: serde_json::Value = serde_json::from_str(&row.sources).unwrap();
    assert_eq!(stored["main.lua"], "function run() return 2 end");
    assert_eq!(stored["helper.lua"], "return 1");
}

#[tokio::test]
async fn save_plugin_source_creates_a_file_that_did_not_exist() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "user.one", &["log"], "function run() end", &["log"], false).await;

    handle_save_plugin_source(
        SavePluginSourceData {
            id: "user.one".to_string(),
            path: "extra.lua".to_string(),
            source: "return {}".to_string(),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    let _ = test.next_frame_json();

    let row = psp_db::plugins::get(&*test.app.driver, "user.one").await.unwrap().unwrap();
    let stored: serde_json::Value = serde_json::from_str(&row.sources).unwrap();
    assert_eq!(stored["extra.lua"], "return {}");
    assert_eq!(stored["main.lua"], "function run() end");
}

#[tokio::test]
async fn save_plugin_source_refuses_a_bundled_plugin() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "bundled.one", &["log"], "function run() end", &["log"], true).await;

    handle_save_plugin_source(
        SavePluginSourceData {
            id: "bundled.one".to_string(),
            path: "main.lua".to_string(),
            source: "function run() return 9 end".to_string(),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "save_plugin_source");
    assert!(!frame["data"]["error"].is_null());

    let row = psp_db::plugins::get(&*test.app.driver, "bundled.one").await.unwrap().unwrap();
    let stored: serde_json::Value = serde_json::from_str(&row.sources).unwrap();
    assert_eq!(stored["main.lua"], "function run() end", "the source must be untouched");
}

#[tokio::test]
async fn saving_the_manifest_stores_it_and_re_grants_from_it() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "user.one", &["log"], "function run() end", &["log"], false).await;

    let widened = manifest_json("user.one", &["log", "save.read"], "run", 1);
    handle_save_plugin_source(
        SavePluginSourceData {
            id: "user.one".to_string(),
            path: "manifest.json".to_string(),
            source: widened,
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "save_plugin_source");

    let row = psp_db::plugins::get(&*test.app.driver, "user.one").await.unwrap().unwrap();
    let granted: Vec<String> = serde_json::from_str(&row.granted_capabilities).unwrap();
    assert!(granted.contains(&"save.read".to_string()), "got {granted:?}");
    assert!(granted.contains(&"log".to_string()), "got {granted:?}");
    let stored: serde_json::Value = serde_json::from_str(&row.manifest).unwrap();
    assert_eq!(stored["id"], "user.one");
}

#[tokio::test]
async fn saving_an_unparsable_manifest_changes_nothing() {
    let mut test = TestContext::new(|_| {}).await;
    let before = seed_row(&test, "user.one", &["log"], "function run() end", &["log"], false).await;

    handle_save_plugin_source(
        SavePluginSourceData {
            id: "user.one".to_string(),
            path: "manifest.json".to_string(),
            source: "{ not json".to_string(),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "save_plugin_source");
    assert!(!frame["data"]["error"].is_null());

    let after = psp_db::plugins::get(&*test.app.driver, "user.one").await.unwrap().unwrap();
    assert_eq!(after.manifest, before.manifest);
    assert_eq!(after.granted_capabilities, before.granted_capabilities);
}

#[tokio::test]
async fn saving_a_manifest_that_requests_save_raw_is_accepted_for_a_user_plugin() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "user.one", &["log"], "function run() end", &["log"], false).await;

    handle_save_plugin_source(
        SavePluginSourceData {
            id: "user.one".to_string(),
            path: "manifest.json".to_string(),
            source: manifest_json("user.one", &["save.raw"], "run", 1),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "save_plugin_source");
    assert_eq!(frame["data"]["error"], serde_json::Value::Null);

    let after = psp_db::plugins::get(&*test.app.driver, "user.one").await.unwrap().unwrap();
    let stored: serde_json::Value = serde_json::from_str(&after.manifest).unwrap();
    assert_eq!(stored["capabilities"], serde_json::json!(["save.raw"]));
}

#[tokio::test]
async fn saving_a_manifest_that_renames_the_plugin_is_refused() {
    let mut test = TestContext::new(|_| {}).await;
    let before = seed_row(&test, "user.one", &["log"], "function run() end", &["log"], false).await;

    handle_save_plugin_source(
        SavePluginSourceData {
            id: "user.one".to_string(),
            path: "manifest.json".to_string(),
            source: manifest_json("other.thing", &["log"], "run", 1),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "save_plugin_source");
    assert!(!frame["data"]["error"].is_null());

    let after = psp_db::plugins::get(&*test.app.driver, "user.one").await.unwrap().unwrap();
    assert_eq!(after.manifest, before.manifest);
    assert!(psp_db::plugins::get(&*test.app.driver, "other.thing").await.unwrap().is_none());
}

#[tokio::test]
async fn save_plugin_source_refuses_an_unknown_plugin() {
    let mut test = TestContext::new(|_| {}).await;
    handle_save_plugin_source(
        SavePluginSourceData {
            id: "nothing.here".to_string(),
            path: "main.lua".to_string(),
            source: "return 1".to_string(),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "save_plugin_source");
    assert!(!frame["data"]["error"].is_null());
}

fn draft(
    id: &str,
    command: &str,
    sources: &[(&str, &str)],
) -> RunPluginDraftData {
    RunPluginDraftData {
        plugin_id: id.to_string(),
        command_id: command.to_string(),
        args: serde_json::Value::Null,
        dry_run: false,
        sources: sources
            .iter()
            .map(|(path, body)| (path.to_string(), body.to_string()))
            .collect(),
        manifest: None,
    }
}

#[tokio::test]
async fn run_plugin_draft_runs_the_draft_source_not_the_stored_one() {
    let mut test = TestContext::new(|_| {}).await;
    test.session.save = Some(minimal_save());
    seed_row(&test, "user.one", &["log"], "function run() return 'stored' end", &["log"], false).await;

    handle_run_plugin_draft(
        draft("user.one", "run", &[("main.lua", "function run() return 'draft' end")]),
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "plugin_run_result");
    assert_eq!(frame["data"]["status"], "ok");
    assert_eq!(frame["data"]["summary"], "draft");
}

#[tokio::test]
async fn run_plugin_draft_does_not_persist_the_draft() {
    let mut test = TestContext::new(|_| {}).await;
    test.session.save = Some(minimal_save());
    let before = seed_row(&test, "user.one", &["log"], "function run() return 'stored' end", &["log"], false).await;

    handle_run_plugin_draft(
        draft("user.one", "run", &[("main.lua", "function run() return 'draft' end")]),
        &mut ctx(&mut test),
    )
    .await
    .unwrap();
    let _ = test.next_frame_json();

    let after = psp_db::plugins::get(&*test.app.driver, "user.one").await.unwrap().unwrap();
    assert_eq!(after.sources, before.sources);
    assert_eq!(after.manifest, before.manifest);
}

#[tokio::test]
async fn a_draft_manifest_cannot_widen_the_stored_grant() {
    let mut test = TestContext::new(|_| {}).await;
    test.session.save = Some(minimal_save());
    seed_row(
        &test,
        "user.one",
        &["log"],
        "function run() return 'stored' end",
        &["log"],
        false,
    )
    .await;

    let mut request = draft(
        "user.one",
        "run",
        &[("main.lua", "function run() return tostring(save ~= nil) end")],
    );
    request.manifest = Some(manifest_json("user.one", &["log", "save.read"], "run", 1));

    handle_run_plugin_draft(request, &mut ctx(&mut test)).await.unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["data"]["status"], "ok");
    assert_eq!(
        frame["data"]["summary"], "false",
        "save.read is not in the stored grant, so `save` must not be installed"
    );
}

#[tokio::test]
async fn a_draft_manifest_may_narrow_the_stored_grant() {
    let mut test = TestContext::new(|_| {}).await;
    test.session.save = Some(minimal_save());
    seed_row(
        &test,
        "user.one",
        &["log", "save.read"],
        "function run() end",
        &["log", "save.read"],
        false,
    )
    .await;

    let mut request = draft(
        "user.one",
        "run",
        &[("main.lua", "function run() return tostring(save ~= nil) end")],
    );
    request.manifest = Some(manifest_json("user.one", &["log"], "run", 1));

    handle_run_plugin_draft(request, &mut ctx(&mut test)).await.unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["data"]["summary"], "false");
}

#[tokio::test]
async fn a_draft_manifest_claiming_another_id_still_uses_the_requested_rows_grant() {
    let mut test = TestContext::new(|_| {}).await;
    test.session.save = Some(minimal_save());
    seed_row(&test, "narrow.one", &["log"], "function run() end", &["log"], false).await;
    seed_row(
        &test,
        "wide.one",
        &["log", "save.read"],
        "function run() end",
        &["log", "save.read"],
        false,
    )
    .await;

    let mut request = draft(
        "narrow.one",
        "run",
        &[("main.lua", "function run() return tostring(save ~= nil) end")],
    );
    request.manifest = Some(manifest_json("wide.one", &["log", "save.read"], "run", 1));

    handle_run_plugin_draft(request, &mut ctx(&mut test)).await.unwrap();

    let frame = test.next_frame_json();
    assert_eq!(
        frame["data"]["summary"], "false",
        "the grant must come from narrow.one, the id the request named"
    );
}

#[tokio::test]
async fn a_user_plugins_draft_manifest_may_claim_save_raw() {
    let mut test = TestContext::new(|_| {}).await;
    test.session.save = Some(minimal_save());
    seed_row(&test, "user.one", &["log"], "function run() end", &["log", "save.raw"], false).await;

    let mut request = draft("user.one", "run", &[("main.lua", "function run() end")]);
    request.manifest = Some(manifest_json("user.one", &["save.raw"], "run", 1));

    handle_run_plugin_draft(request, &mut ctx(&mut test)).await.unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["data"]["status"], "ok");
}

#[tokio::test]
async fn run_plugin_draft_runs_a_disabled_plugin() {
    let mut test = TestContext::new(|_| {}).await;
    test.session.save = Some(minimal_save());
    seed_row(&test, "user.one", &["log"], "function run() return 'ok' end", &["log"], false).await;
    psp_db::plugins::set_enabled(&*test.app.driver, "user.one", false).await.unwrap();

    handle_run_plugin_draft(
        draft("user.one", "run", &[("main.lua", "function run() return 'ok' end")]),
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["data"]["status"], "ok");
    assert_eq!(frame["data"]["summary"], "ok");
}

#[tokio::test]
async fn run_plugin_draft_refuses_an_unknown_plugin() {
    let mut test = TestContext::new(|_| {}).await;
    handle_run_plugin_draft(
        draft("nothing.here", "run", &[("main.lua", "function run() end")]),
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "plugin_run_result");
    assert_eq!(frame["data"]["status"], "error");
}

#[tokio::test]
async fn run_plugin_draft_reports_a_syntax_error_as_a_run_error() {
    let mut test = TestContext::new(|_| {}).await;
    test.session.save = Some(minimal_save());
    seed_row(&test, "user.one", &["log"], "function run() end", &["log"], false).await;

    handle_run_plugin_draft(
        draft("user.one", "run", &[("main.lua", "function run( end")]),
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["data"]["status"], "error");
}

#[tokio::test]
async fn run_plugin_draft_refuses_a_bundled_plugin() {
    let mut test = TestContext::new(|_| {}).await;
    test.session.save = Some(minimal_save());
    let before = seed_row(
        &test,
        "bundled.one",
        &["log"],
        "function run() return 'stored' end",
        &["log"],
        true,
    )
    .await;

    handle_run_plugin_draft(
        draft("bundled.one", "run", &[("main.lua", "function run() return 'draft' end")]),
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "plugin_run_result");
    assert_eq!(frame["data"]["status"], "error");
    assert_eq!(frame["data"]["summary"], serde_json::Value::Null);

    let after = psp_db::plugins::get(&*test.app.driver, "bundled.one").await.unwrap().unwrap();
    assert_eq!(after.sources, before.sources, "the stored sources must be untouched");
    assert_eq!(after.manifest, before.manifest);
}

#[tokio::test]
async fn a_bundled_rows_draft_cannot_borrow_its_save_raw_grant() {
    let mut test = TestContext::new(|_| {}).await;
    test.session.save = Some(minimal_save());
    seed_row(&test, "bundled.one", &["save.raw"], "function run() end", &["save.raw"], true).await;

    let mut request = draft(
        "bundled.one",
        "run",
        &[("main.lua", "function run() return tostring(raw ~= nil) end")],
    );
    request.manifest = Some(manifest_json("bundled.one", &["save.raw"], "run", 1));

    handle_run_plugin_draft(request, &mut ctx(&mut test)).await.unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["data"]["status"], "error");
    assert_eq!(
        frame["data"]["summary"],
        serde_json::Value::Null,
        "the draft must never have run"
    );
}

#[tokio::test]
async fn delete_plugin_source_removes_a_non_entry_file() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "user.multi", &[], "function run() end", &[], false).await;
    psp_db::plugins::set_sources(
        &*test.app.driver,
        "user.multi",
        &serde_json::json!({ "main.lua": "function run() end", "lib/util.lua": "return {}" })
            .to_string(),
    )
    .await
    .unwrap();

    handle_delete_plugin_source(
        DeletePluginSourceData {
            id: "user.multi".to_string(),
            path: "lib/util.lua".to_string(),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "delete_plugin_source");
    assert_eq!(frame["data"]["path"], "lib/util.lua");

    let row = psp_db::plugins::get(&*test.app.driver, "user.multi").await.unwrap().unwrap();
    let stored: serde_json::Value = serde_json::from_str(&row.sources).unwrap();
    assert!(stored.get("lib/util.lua").is_none());
    assert!(stored.get("main.lua").is_some(), "the entry must survive");
}

#[tokio::test]
async fn delete_plugin_source_refuses_the_entry() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "user.multi", &[], "function run() end", &[], false).await;

    handle_delete_plugin_source(
        DeletePluginSourceData { id: "user.multi".to_string(), path: "main.lua".to_string() },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "delete_plugin_source");
    assert!(
        frame["data"]["error"].as_str().unwrap_or_default().contains("entry"),
        "the refusal must say why, got {frame}"
    );

    let row = psp_db::plugins::get(&*test.app.driver, "user.multi").await.unwrap().unwrap();
    let stored: serde_json::Value = serde_json::from_str(&row.sources).unwrap();
    assert!(stored.get("main.lua").is_some());
}

#[tokio::test]
async fn delete_plugin_source_refuses_a_bundled_plugin() {
    let mut test = TestContext::new(|_| {}).await;
    seed_row(&test, "pst.demo", &[], "function run() end", &[], true).await;

    handle_delete_plugin_source(
        DeletePluginSourceData { id: "pst.demo".to_string(), path: "lib/util.lua".to_string() },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "delete_plugin_source");
    assert!(
        frame["data"]["error"].as_str().unwrap_or_default().contains("bundled"),
        "the refusal must say why, got {frame}"
    );
}
