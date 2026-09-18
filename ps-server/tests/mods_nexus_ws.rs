mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::{json, Value};

use common::nexus_stub::{self, NexusStub, FILE_ID, FREE_KEY, MOD_ID, PREMIUM_KEY};
use ps_server::desktop_dialogs::NullDialogProvider;
use ps_server::services::docker::mock::MockDocker;
use ps_server::services::nexus::api::HttpNexusApi;
use ps_server::services::nexus::keystore::{MemoryKeyStore, NexusKeyStore};
use ps_server::services::nexus::protocol::{HandlerStatus, ProtocolRegistry, RegistryError};
use ps_server::services::ServerServices;

const OURS: &str = r#""C:\PalStudio\palstudio.exe" "%1""#;
const VORTEX: &str = r#""C:\Vortex\Vortex.exe" -d "%1""#;

#[derive(Default)]
struct FakeRegistry {
    current: std::sync::Mutex<Option<String>>,
    registrations: std::sync::atomic::AtomicUsize,
}

impl ProtocolRegistry for FakeRegistry {
    fn status(&self) -> Result<HandlerStatus, RegistryError> {
        let current = self.current.lock().unwrap().clone();
        let registered = current.as_deref() == Some(OURS);
        Ok(HandlerStatus {
            supported: true,
            registered,
            foreign: current.is_some() && !registered,
            current,
        })
    }

    fn register(&self) -> Result<HandlerStatus, RegistryError> {
        self.registrations
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        *self.current.lock().unwrap() = Some(OURS.to_string());
        self.status()
    }
}

struct NexusServer {
    server: common::TestServer,
    stub: NexusStub,
    keys: Arc<MemoryKeyStore>,
    registry: Arc<FakeRegistry>,
}

async fn start_nexus_server(key: Option<&str>) -> NexusServer {
    let stub = nexus_stub::spawn_nexus_stub().await;
    let keys = Arc::new(key.map(MemoryKeyStore::with_key).unwrap_or_default());
    let registry = Arc::new(FakeRegistry::default());
    let docker = Arc::new(MockDocker::default());
    let mut services = ServerServices::with_docker(
        docker,
        std::env::temp_dir().join("ps-nexus-ws-test-unused-app-root"),
    );
    services.nexus = Arc::new(HttpNexusApi::new(&stub.base));
    services.nexus_keys = keys.clone();
    services.protocol_registry = registry.clone();
    let dialogs: Arc<dyn ps_server::desktop_dialogs::FileDialogProvider> =
        Arc::new(NullDialogProvider);
    let server = common::start_desktop_test_server_with_services(dialogs, services).await;
    NexusServer { server, stub, keys, registry }
}

async fn request(ws: &mut common::WsClient, message_type: &str, data: Value) -> Value {
    common::send_json(ws, json!({ "type": message_type, "data": data })).await;
    common::mods_ws::next_of_type(ws, message_type).await.0
}

fn stored_key(keys: &MemoryKeyStore) -> Option<String> {
    keys.get().unwrap().map(|key| key.expose().to_string())
}

#[tokio::test]
async fn nexus_messages_refuse_outside_the_desktop_app() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    for (message_type, data) in [
        ("nexus_account_get", Value::Null),
        ("nexus_key_set", json!({ "key": "abc" })),
        ("nexus_key_clear", Value::Null),
        ("nexus_categories", Value::Null),
        ("nexus_search", json!({})),
        ("nexus_mod_files", json!({ "mod_id": 1 })),
        ("nexus_download", json!({ "target_id": "x", "mod_id": 1, "file_id": 2 })),
        ("nexus_link_subscribe", Value::Null),
        ("nexus_handler_status", Value::Null),
        ("nexus_handler_register", Value::Null),
        ("mod_update_check", Value::Null),
        ("mod_update_ignore", json!({ "mod_id": "x", "version": null })),
    ] {
        let reply = request(&mut ws, message_type, data).await;
        assert_eq!(reply["data"]["error"]["code"], "desktop_only", "{message_type}: {reply}");
    }
    server.handle.shutdown().await;
}

#[tokio::test]
async fn links_wait_for_a_subscriber_and_then_arrive_live() {
    let nexus = start_nexus_server(None).await;
    let links = nexus.server.handle.services.nexus_links.clone();
    links.push_raw("nxm://palworld/mods/4821/files/99001?key=abc&expires=4102444800&user_id=7");

    let mut ws = common::connect(&nexus.server).await;
    let reply = request(&mut ws, "nexus_link_subscribe", Value::Null).await;
    assert_eq!(reply["data"]["active"], true, "{reply}");
    let queued = common::next_json(&mut ws).await;
    assert_eq!(queued["type"], "nexus_link", "{queued}");
    assert_eq!(queued["data"]["link"]["mod_id"], 4821);
    assert_eq!(queued["data"]["link"]["file_id"], 99001);
    assert_eq!(queued["data"]["link"]["key"], "abc");
    assert_eq!(queued["data"]["link"]["expires"], 4_102_444_800u64);
    assert!(queued["data"]["error"].is_null());

    let again = request(&mut ws, "nexus_link_subscribe", Value::Null).await;
    assert_eq!(again["data"]["active"], true);
    links.push_raw("nxm://palworld/collections/tidy/revisions/2");
    let rejected = common::next_json(&mut ws).await;
    assert_eq!(rejected["data"]["error"]["code"], "unsupported_link", "{rejected}");
    links.push_raw("nxm://palworld/mods/4821/files/99001?key=abc&expires=1");
    let expired = common::next_json(&mut ws).await;
    assert_eq!(expired["data"]["error"]["code"], "link_expired", "{expired}");

    common::send_json(&mut ws, json!({ "type": "mod_list", "data": null })).await;
    let next = common::next_json(&mut ws).await;
    assert_eq!(next["type"], "mod_list", "no duplicate nexus_link frames: {next}");
    assert_eq!(links.pending_len(), 0);
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn without_a_key_the_account_is_empty() {
    let nexus = start_nexus_server(None).await;
    let mut ws = common::connect(&nexus.server).await;
    let reply = request(&mut ws, "nexus_account_get", Value::Null).await;
    assert_eq!(reply["data"]["has_key"], false, "{reply}");
    assert!(reply["data"]["account"].is_null());
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn a_rejected_key_is_not_stored_or_echoed() {
    let nexus = start_nexus_server(None).await;
    let mut ws = common::connect(&nexus.server).await;
    let reply = request(&mut ws, "nexus_key_set", json!({ "key": "not-a-real-key" })).await;
    assert_eq!(reply["data"]["error"]["code"], "invalid_key", "{reply}");
    assert!(!reply.to_string().contains("not-a-real-key"));
    assert_eq!(stored_key(&nexus.keys), None);
    for data in [json!({ "key": "   " }), json!({}), Value::Null] {
        let blank = request(&mut ws, "nexus_key_set", data).await;
        assert_eq!(blank["data"]["error"]["code"], "invalid_key", "{blank}");
    }
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn a_valid_key_is_stored_and_the_account_comes_back_without_it() {
    let nexus = start_nexus_server(None).await;
    let mut ws = common::connect(&nexus.server).await;

    let set =
        request(&mut ws, "nexus_key_set", json!({ "key": format!("  {PREMIUM_KEY} ") })).await;
    assert_eq!(set["data"]["has_key"], true, "{set}");
    assert_eq!(set["data"]["account"]["name"], "Tester");
    assert_eq!(set["data"]["account"]["is_premium"], true);
    assert!(!set.to_string().contains(PREMIUM_KEY));
    assert_eq!(stored_key(&nexus.keys).as_deref(), Some(PREMIUM_KEY));

    let account = request(&mut ws, "nexus_account_get", Value::Null).await;
    assert_eq!(account["data"]["account"]["user_id"], 7, "{account}");
    assert!(!account.to_string().contains(PREMIUM_KEY));

    let cleared = request(&mut ws, "nexus_key_clear", Value::Null).await;
    assert_eq!(cleared["data"]["has_key"], false, "{cleared}");
    assert_eq!(stored_key(&nexus.keys), None);
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn categories_need_a_key() {
    let without = start_nexus_server(None).await;
    let mut ws = common::connect(&without.server).await;
    let refused = request(&mut ws, "nexus_categories", Value::Null).await;
    assert_eq!(refused["data"]["error"]["code"], "key_required", "{refused}");
    without.server.handle.shutdown().await;

    let with = start_nexus_server(Some(FREE_KEY)).await;
    let mut ws = common::connect(&with.server).await;
    let reply = request(&mut ws, "nexus_categories", Value::Null).await;
    let categories = reply["data"]["categories"].as_array().unwrap();
    assert_eq!(categories.len(), 2, "{reply}");
    assert!(categories[0]["parent_category"].is_null());
    assert_eq!(categories[1]["parent_category"], 1);
    with.server.handle.shutdown().await;
}

#[tokio::test]
async fn search_and_file_lists_are_anonymous_even_with_a_key() {
    let nexus = start_nexus_server(Some(PREMIUM_KEY)).await;
    let mut ws = common::connect(&nexus.server).await;

    let search =
        request(&mut ws, "nexus_search", json!({ "query": "cool", "offset": 0, "count": 10 }))
            .await;
    assert_eq!(search["data"]["mods"][0]["mod_id"], MOD_ID, "{search}");
    assert_eq!(search["data"]["total_count"], 1);
    assert_eq!(search["data"]["count"], 1);

    let files = request(&mut ws, "nexus_mod_files", json!({ "mod_id": MOD_ID })).await;
    assert_eq!(files["data"]["mod_id"], MOD_ID, "{files}");
    assert_eq!(files["data"]["files"].as_array().unwrap().len(), 2);
    assert_eq!(files["data"]["latest_file_id"], nexus_stub::FILE_ID);
    assert!(files["data"]["files"][1]["size_in_bytes"].is_u64());

    let keys = nexus.stub.state.graphql_apikeys.lock().unwrap().clone();
    assert_eq!(keys, vec![None, None]);
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn an_exhausted_rate_limit_refuses_with_its_reset() {
    let nexus = start_nexus_server(Some(PREMIUM_KEY)).await;
    nexus.stub.state.rate_limited.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut ws = common::connect(&nexus.server).await;
    let reply = request(&mut ws, "nexus_account_get", Value::Null).await;
    assert_eq!(reply["data"]["error"]["code"], "rate_limited", "{reply}");
    assert_eq!(reply["data"]["error"]["reset"], "2026-09-17T00:00:00+00:00");
    assert_eq!(reply["data"]["has_key"], true);
    nexus.server.handle.shutdown().await;
}

async fn add_target(ws: &mut common::WsClient, root: &Path) -> String {
    let reply = request(ws, "mod_target_add", json!({ "root_path": root.to_string_lossy() })).await;
    reply["data"]["target"]["id"].as_str().expect("a target id").to_string()
}

async fn download(ws: &mut common::WsClient, data: Value) -> (Value, Vec<Value>) {
    common::send_json(ws, json!({ "type": "nexus_download", "data": data })).await;
    common::mods_ws::next_of_type(ws, "nexus_download").await
}

fn stages(skipped: &[Value]) -> Vec<String> {
    skipped
        .iter()
        .filter(|frame| frame["type"] == "mod_progress")
        .map(|frame| frame["data"]["stage"].as_str().unwrap().to_string())
        .collect()
}

fn download_dir(server: &common::TestServer, file_id: u32) -> PathBuf {
    server
        ._temp_dir
        .path()
        .join("downloads")
        .join("nexus")
        .join(format!("{MOD_ID}-{file_id}"))
}

async fn library_mod(ws: &mut common::WsClient, id: &str) -> Option<Value> {
    let listed = request(ws, "mod_list", Value::Null).await;
    listed["data"]["mods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .cloned()
}

#[tokio::test]
async fn a_premium_account_downloads_and_installs_a_nexus_file() {
    let nexus = start_nexus_server(Some(PREMIUM_KEY)).await;
    let mut ws = common::connect(&nexus.server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let (reply, skipped) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    let data = &reply["data"];
    assert!(data["error"].is_null(), "{reply}");
    assert_eq!(data["mod_id"], "nexus-4821");
    assert_eq!(data["version_id"], "nexus-4821@1.2.0");
    assert_eq!(data["nexus_mod_id"], MOD_ID);
    assert_eq!(data["file_id"], FILE_ID);
    assert_eq!(data["version"], "1.2.0");
    assert_eq!(data["file_name"], nexus_stub::file_uri(FILE_ID, "1.2.0"));
    assert_eq!(data["target_id"], target_id);
    assert!(data["enable_error"].is_null(), "{reply}");
    let stages = stages(&skipped);
    for stage in ["resolving", "downloading", "installing"] {
        assert!(stages.iter().any(|seen| seen == stage), "{stages:?}");
    }
    assert_eq!(nexus.stub.state.cdn_hits.load(Ordering::SeqCst), 1);
    let queries = nexus.stub.state.download_link_queries.lock().unwrap().clone();
    assert_eq!(queries.len(), 1);
    assert!(!queries[0].contains_key("key"));
    assert!(!download_dir(&nexus.server, FILE_ID).exists());

    let row = library_mod(&mut ws, "nexus-4821").await.expect("the mod is listed");
    assert_eq!(row["source_kind"], "nexus");
    assert_eq!(row["nexus_mod_id"], MOD_ID);

    let (again, _) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    assert_eq!(again["data"]["error"]["code"], "already_installed", "{again}");
    assert!(!download_dir(&nexus.server, FILE_ID).exists());
    // The already-installed check runs before `download_link` or the CDN, so
    // a repeat request must not add to either count.
    assert_eq!(nexus.stub.state.cdn_hits.load(Ordering::SeqCst), 1);
    assert_eq!(nexus.stub.state.download_link_queries.lock().unwrap().len(), 1);
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn a_free_account_needs_an_nxm_link() {
    let nexus = start_nexus_server(Some(FREE_KEY)).await;
    let mut ws = common::connect(&nexus.server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let (refused, _) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    assert_eq!(refused["data"]["error"]["code"], "premium_required", "{refused}");
    assert_eq!(nexus.stub.state.cdn_hits.load(Ordering::SeqCst), 0);
    assert!(library_mod(&mut ws, "nexus-4821").await.is_none());
    assert!(!download_dir(&nexus.server, FILE_ID).exists());

    let (installed, _) = download(
        &mut ws,
        json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID,
                "key": "nxm-key", "expires": 4_102_444_800u64 }),
    )
    .await;
    assert_eq!(installed["data"]["mod_id"], "nexus-4821", "{installed}");
    let queries = nexus.stub.state.download_link_queries.lock().unwrap().clone();
    assert_eq!(queries[1].get("key").map(String::as_str), Some("nxm-key"));
    assert_eq!(queries[1].get("expires").map(String::as_str), Some("4102444800"));
    assert!(!installed.to_string().contains("nxm-key"));
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn link_and_key_problems_refuse_before_downloading() {
    let nexus = start_nexus_server(None).await;
    let mut ws = common::connect(&nexus.server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let (expired, _) = download(
        &mut ws,
        json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID, "key": "k", "expires": 1 }),
    )
    .await;
    assert_eq!(expired["data"]["error"]["code"], "link_expired", "{expired}");

    let (half, _) = download(
        &mut ws,
        json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID, "key": "k" }),
    )
    .await;
    assert_eq!(half["data"]["error"]["code"], "invalid_link", "{half}");

    let (no_key, _) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    assert_eq!(no_key["data"]["error"]["code"], "key_required", "{no_key}");

    let (unknown, _) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": 5 })).await;
    assert_eq!(unknown["data"]["error"]["code"], "not_found", "{unknown}");

    let (no_target, _) =
        download(&mut ws, json!({ "target_id": "client-nope", "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    assert_eq!(no_target["data"]["error"]["code"], "target_not_found", "{no_target}");

    assert!(nexus.stub.state.download_link_queries.lock().unwrap().is_empty());
    assert_eq!(nexus.stub.state.cdn_hits.load(Ordering::SeqCst), 0);
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn an_archive_already_downloaded_is_installed_without_a_key() {
    let nexus = start_nexus_server(None).await;
    let mut ws = common::connect(&nexus.server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let dir = download_dir(&nexus.server, FILE_ID);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(nexus_stub::file_uri(FILE_ID, "1.2.0")), nexus_stub::stub_archive()).unwrap();

    let (reply, _) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    assert_eq!(reply["data"]["version_id"], "nexus-4821@1.2.0", "{reply}");
    assert!(nexus.stub.state.download_link_queries.lock().unwrap().is_empty());
    assert_eq!(nexus.stub.state.cdn_hits.load(Ordering::SeqCst), 0);
    assert!(!dir.exists());
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn an_already_installed_mod_refuses_without_a_download_link_query() {
    let nexus = start_nexus_server(Some(PREMIUM_KEY)).await;
    let mut ws = common::connect(&nexus.server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let (first, _) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    assert!(first["data"]["error"].is_null(), "{first}");
    assert_eq!(nexus.stub.state.cdn_hits.load(Ordering::SeqCst), 1);
    assert_eq!(nexus.stub.state.download_link_queries.lock().unwrap().len(), 1);

    let (second, _) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    assert_eq!(second["data"]["error"]["code"], "already_installed", "{second}");
    assert_eq!(second["data"]["error"]["mod_id"], "nexus-4821", "{second}");
    assert_eq!(second["data"]["error"]["version_id"], "nexus-4821@1.2.0", "{second}");
    // No new Nexus request quota spent and no fresh archive download for a
    // mod version the library already holds.
    assert_eq!(nexus.stub.state.cdn_hits.load(Ordering::SeqCst), 1);
    assert_eq!(nexus.stub.state.download_link_queries.lock().unwrap().len(), 1);
    assert!(!download_dir(&nexus.server, FILE_ID).exists());
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn a_blank_version_skips_the_pre_check_and_a_refusal_keeps_the_archive() {
    let nexus = start_nexus_server(Some(PREMIUM_KEY)).await;
    let mut ws = common::connect(&nexus.server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let size = nexus.stub.state.archive.len();
    nexus
        .stub
        .state
        .files
        .lock()
        .unwrap()
        .insert(MOD_ID, json!([nexus_stub::file_json(FILE_ID, "MAIN", "", size)]));

    let (first, _) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    assert!(first["data"]["error"].is_null(), "{first}");
    assert_eq!(first["data"]["version_id"], "nexus-4821@unversioned", "{first}");
    assert_eq!(nexus.stub.state.download_link_queries.lock().unwrap().len(), 1);
    assert_eq!(nexus.stub.state.cdn_hits.load(Ordering::SeqCst), 1);
    assert!(!download_dir(&nexus.server, FILE_ID).exists());

    // A blank version cannot be looked up by the pre-check: the install path
    // falls back to the manifest's own version instead, which this pre-check
    // has no way to predict. The repeat therefore still spends a
    // `download_link` query and a CDN hit before the library's own duplicate
    // check refuses it once the archive is staged again.
    let (second, _) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    assert_eq!(second["data"]["error"]["code"], "already_installed", "{second}");
    assert_eq!(nexus.stub.state.download_link_queries.lock().unwrap().len(), 2);
    assert_eq!(nexus.stub.state.cdn_hits.load(Ordering::SeqCst), 2);
    // Refused, not Installed: the downloaded archive is kept, not deleted.
    assert!(download_dir(&nexus.server, FILE_ID).exists());
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn registration_reports_a_foreign_handler_before_replacing_it() {
    let nexus = start_nexus_server(None).await;
    *nexus.registry.current.lock().unwrap() = Some(VORTEX.to_string());
    let mut ws = common::connect(&nexus.server).await;

    let status = request(&mut ws, "nexus_handler_status", Value::Null).await;
    assert_eq!(status["data"]["foreign"], true, "{status}");
    assert_eq!(status["data"]["registered"], false);
    assert_eq!(status["data"]["current"], VORTEX);

    let refused = request(&mut ws, "nexus_handler_register", json!({})).await;
    assert_eq!(refused["data"]["error"]["code"], "foreign_handler", "{refused}");
    assert_eq!(refused["data"]["error"]["current"], VORTEX);
    assert_eq!(nexus.registry.registrations.load(std::sync::atomic::Ordering::SeqCst), 0);

    let forced = request(&mut ws, "nexus_handler_register", json!({ "force": true })).await;
    assert_eq!(forced["data"]["registered"], true, "{forced}");
    assert_eq!(forced["data"]["foreign"], false);
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn registering_with_no_handler_needs_no_force() {
    let nexus = start_nexus_server(None).await;
    let mut ws = common::connect(&nexus.server).await;
    let reply = request(&mut ws, "nexus_handler_register", Value::Null).await;
    assert_eq!(reply["data"]["registered"], true, "{reply}");
    assert_eq!(nexus.registry.registrations.load(std::sync::atomic::Ordering::SeqCst), 1);
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn update_checks_find_newer_main_files_and_honour_ignores() {
    let nexus = start_nexus_server(Some(PREMIUM_KEY)).await;
    let mut ws = common::connect(&nexus.server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let (installed, _) =
        download(&mut ws, json!({ "target_id": target_id, "mod_id": MOD_ID, "file_id": FILE_ID })).await;
    assert_eq!(installed["data"]["version_id"], "nexus-4821@1.2.0", "{installed}");
    {
        let size = nexus.stub.state.archive.len();
        nexus.stub.state.files.lock().unwrap().insert(
            MOD_ID,
            json!([
                nexus_stub::file_json(FILE_ID, "MAIN", "1.2.0", size),
                nexus_stub::file_json(99002, "UPDATE", "1.3.0", size),
                nexus_stub::file_json(99003, "OPTIONAL", "9.9.9", size),
            ]),
        );
    }
    let graphql_before = nexus.stub.state.graphql_apikeys.lock().unwrap().len();

    let checked = request(&mut ws, "mod_update_check", json!({})).await;
    let updates = checked["data"]["updates"].as_array().unwrap().clone();
    assert_eq!(updates.len(), 1, "{checked}");
    assert_eq!(updates[0]["mod_id"], "nexus-4821");
    assert_eq!(updates[0]["nexus_mod_id"], MOD_ID);
    assert_eq!(updates[0]["installed_version"], "1.2.0");
    assert_eq!(updates[0]["latest"]["file_id"], 99002);
    assert_eq!(updates[0]["state"], "available");
    assert_eq!(checked["data"]["checked"], 1);
    assert_eq!(checked["data"]["truncated"], false);

    let ignored = request(&mut ws, "mod_update_ignore", json!({ "mod_id": "nexus-4821", "version": "1.3.0" })).await;
    assert_eq!(ignored["data"]["ignored_version"], "1.3.0", "{ignored}");
    let scoped = request(&mut ws, "mod_update_check", json!({ "target_id": target_id })).await;
    assert_eq!(scoped["data"]["target_id"], target_id);
    assert_eq!(scoped["data"]["updates"][0]["state"], "ignored", "{scoped}");
    assert_eq!(scoped["data"]["updates"][0]["ignored_version"], "1.3.0");

    let cleared = request(&mut ws, "mod_update_ignore", json!({ "mod_id": "nexus-4821", "version": null })).await;
    assert!(cleared["data"]["ignored_version"].is_null(), "{cleared}");
    let again = request(&mut ws, "mod_update_check", Value::Null).await;
    assert_eq!(again["data"]["updates"][0]["state"], "available", "{again}");

    // A blank version also clears the ignore, the same as an omitted one.
    let ignored_again =
        request(&mut ws, "mod_update_ignore", json!({ "mod_id": "nexus-4821", "version": "1.3.0" })).await;
    assert_eq!(ignored_again["data"]["ignored_version"], "1.3.0", "{ignored_again}");
    let cleared_blank =
        request(&mut ws, "mod_update_ignore", json!({ "mod_id": "nexus-4821", "version": "" })).await;
    assert!(cleared_blank["data"]["ignored_version"].is_null(), "{cleared_blank}");
    let after_blank_clear = request(&mut ws, "mod_update_check", Value::Null).await;
    assert_eq!(after_blank_clear["data"]["updates"][0]["state"], "available", "{after_blank_clear}");

    let unknown = request(&mut ws, "mod_update_ignore", json!({ "mod_id": "nope", "version": "1" })).await;
    assert_eq!(unknown["data"]["error"]["code"], "mod_not_found", "{unknown}");
    let no_target = request(&mut ws, "mod_update_check", json!({ "target_id": "client-nope" })).await;
    assert_eq!(no_target["data"]["error"]["code"], "target_not_found", "{no_target}");

    let keys = nexus.stub.state.graphql_apikeys.lock().unwrap().clone();
    assert!(keys[graphql_before..].iter().all(Option::is_none));
    nexus.server.handle.shutdown().await;
}

#[tokio::test]
async fn update_checks_cover_at_most_fifty_mods() {
    let nexus = start_nexus_server(None).await;
    let db = &*nexus.server.handle.app.driver;
    for n in 1..=51u32 {
        let id = format!("nexus-{n}");
        ps_db::mod_library::upsert_mod(
            db,
            &ps_db::mod_library::NewMod {
                id: id.clone(),
                name: format!("Mod {n}"),
                mod_type: "ue4ss".to_string(),
                source_kind: "nexus".to_string(),
                source_ref: "{}".to_string(),
                nexus_mod_id: Some(i64::from(n)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        ps_db::mod_library::insert_version(
            db,
            &ps_db::mod_library::NewModVersion {
                id: format!("{id}@1.0"),
                mod_id: id.clone(),
                version: "1.0".to_string(),
                library_dir: nexus.server._temp_dir.path().join("unused").to_string_lossy().into_owned(),
                manifest: r#"{"routes":[]}"#.to_string(),
                source_ref: "{}".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }
    let mut ws = common::connect(&nexus.server).await;
    let reply = request(&mut ws, "mod_update_check", json!({})).await;
    assert_eq!(reply["data"]["checked"], 50, "{reply}");
    assert_eq!(reply["data"]["truncated"], true);
    let queries = nexus.stub.state.graphql_queries.lock().unwrap().clone();
    assert_eq!(queries.len(), 1, "one batched request");
    assert_eq!(queries[0].matches("modFiles(").count(), 50);
    nexus.server.handle.shutdown().await;
}
