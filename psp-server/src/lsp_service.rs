//! The real `LspService`: acquires `lua-language-server`, materialises a
//! workspace per plugin, and keeps one initialised child process per plugin id.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use psp_app::lsp::{LspService, TierStatus};

use crate::emitter::Emitter;
use crate::services::language_server;
use crate::services::lsp_process::{ClientSlot, LspProcess};
use crate::services::lsp_workspace::materialise;

const UNSUPPORTED_HOST: &str = "no lua-language-server release is pinned for this platform";

const TIER_AVAILABLE: u8 = 0;
const TIER_STARTING: u8 = 1;
const TIER_UNAVAILABLE: u8 = 2;

/// The state of the deployment's `lua-language-server` binary, and nothing
/// else. `status()` reads it with one atomic load, so the answer can never
/// wait on a download.
///
/// It describes the binary rather than any one plugin: `Unavailable` when no
/// release is pinned for this host or acquiring it failed, `Starting` while an
/// acquisition is in flight, `Available` once it is on disk. A failure that
/// belongs to a single plugin — unwritable sources, a child that would not
/// spawn — must never move it, because one plugin's problem is not the
/// deployment's tier.
///
/// `Available` therefore means the full tier is on offer here, not that a
/// child is currently up. `get_editor_tier` is the probe a client uses to
/// decide whether to attempt the full tier at all, and answering `baseline`
/// before anything has been asked of the service would strand every client on
/// the baseline editor forever.
struct Tier {
    code: AtomicU8,
    reason: Mutex<String>,
    in_flight: AtomicUsize,
}

impl Tier {
    fn new(code: u8, reason: &str) -> Self {
        Self {
            code: AtomicU8::new(code),
            reason: Mutex::new(reason.to_string()),
            in_flight: AtomicUsize::new(0),
        }
    }

    fn status(&self) -> TierStatus {
        match self.code.load(Ordering::SeqCst) {
            TIER_STARTING => TierStatus::Starting,
            TIER_UNAVAILABLE => TierStatus::Unavailable {
                reason: self.reason.lock().unwrap().clone(),
            },
            _ => TierStatus::Available,
        }
    }

    fn begin(&self) -> Acquiring<'_> {
        let previous = self.code.load(Ordering::SeqCst);
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        self.code.store(TIER_STARTING, Ordering::SeqCst);
        Acquiring {
            tier: self,
            previous,
            settled: std::cell::Cell::new(false),
        }
    }

    /// Only the last acquisition out settles the tier. A second caller whose
    /// binary was already on disk finishes in microseconds, and letting it
    /// publish `Available` would paint over the `Starting` that a download
    /// still running for another plugin is relying on.
    ///
    /// The reason is written before the code, so a reader that sees
    /// `Unavailable` never picks up the previous failure's reason.
    fn finish(&self, code: u8, reason: Option<&str>) {
        if let Some(reason) = reason {
            *self.reason.lock().unwrap() = reason.to_string();
        }
        if self.in_flight.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.code.store(code, Ordering::SeqCst);
        }
    }

    fn set_unavailable(&self, reason: &str) {
        *self.reason.lock().unwrap() = reason.to_string();
        self.code.store(TIER_UNAVAILABLE, Ordering::SeqCst);
    }
}

/// One in-flight acquisition of the binary. Held across the acquisition so a
/// caller queued behind another's download still reads `Starting`.
struct Acquiring<'a> {
    tier: &'a Tier,
    previous: u8,
    settled: std::cell::Cell<bool>,
}

impl Acquiring<'_> {
    fn settle(&self, code: u8, reason: Option<&str>) {
        if !self.settled.replace(true) {
            self.tier.finish(code, reason);
        }
    }
}

impl Drop for Acquiring<'_> {
    /// Covers a caller cancelled mid-acquisition — a client disconnecting
    /// drops the handler future — which would otherwise leave the count, and
    /// so the tier, stuck on `Starting` for the life of the process. Nothing
    /// was learned about the binary, so the tier goes back to what it said
    /// before.
    fn drop(&mut self) {
        if !self.settled.get() {
            let restore = if self.previous == TIER_STARTING {
                TIER_AVAILABLE
            } else {
                self.previous
            };
            self.tier.finish(restore, None);
        }
    }
}

pub struct ServerLspService {
    install_root: PathBuf,
    workspace_root: PathBuf,
    /// `Arc` rather than a bare `LspProcess` so a request on one plugin can
    /// await its reply without holding the map shut against every other.
    processes: tokio::sync::Mutex<HashMap<String, Arc<LspProcess>>>,
    /// Serialises acquisition, workspace materialisation and spawning.
    /// `language_server::ensure` finishes with a `remove_dir_all` and a
    /// `rename` onto the install root, and `materialise` clears a workspace
    /// before rewriting it; neither survives a concurrent twin. Two editors
    /// opening two plugins at once is the ordinary case, so this is a separate
    /// lock from `processes` — holding that one across a download would stall
    /// every in-flight request on every other plugin.
    acquisition: tokio::sync::Mutex<()>,
    tier: Tier,
    /// Shared with every running child's reader task, which reads it on each
    /// unprompted frame. Replacing its contents redirects a child that is
    /// already up, which is what a page reload needs: the connection changes
    /// underneath a language server that nothing shuts down.
    client: ClientSlot,
}

impl ServerLspService {
    pub fn new(install_root: PathBuf, workspace_root: PathBuf) -> Self {
        let tier = match language_server::release_for_host() {
            Some(_) => Tier::new(TIER_AVAILABLE, ""),
            None => Tier::new(TIER_UNAVAILABLE, UNSUPPORTED_HOST),
        };
        Self {
            install_root,
            workspace_root,
            processes: tokio::sync::Mutex::new(HashMap::new()),
            acquisition: tokio::sync::Mutex::new(()),
            tier,
            client: Arc::new(Mutex::new(None)),
        }
    }

    async fn process(&self, plugin_id: &str) -> Result<Arc<LspProcess>, String> {
        self.processes
            .lock()
            .await
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| {
                format!("no language server is running for plugin {plugin_id}; open it first")
            })
    }

    /// Acquires the binary if it is not on disk, materialises the plugin's
    /// workspace, and leaves an initialised child process in the map.
    /// Returns the workspace it indexed.
    async fn ensure_ready(
        &self,
        plugin_id: &str,
        sources: &BTreeMap<String, String>,
    ) -> Result<PathBuf, String> {
        if language_server::release_for_host().is_none() {
            self.tier.set_unavailable(UNSUPPORTED_HOST);
            return Err(UNSUPPORTED_HOST.to_string());
        }

        let acquiring = self.tier.begin();
        let _serialised = self.acquisition.lock().await;

        let exe = match language_server::ensure(&self.install_root).await {
            Ok(exe) => {
                acquiring.settle(TIER_AVAILABLE, None);
                exe
            }
            Err(error) => {
                let reason = format!("the language server could not be installed: {error}");
                acquiring.settle(TIER_UNAVAILABLE, Some(&reason));
                return Err(reason);
            }
        };

        // Everything below belongs to one plugin, so none of it touches the
        // tier: the binary is on disk either way.
        let workspace = materialise(&self.workspace_root, plugin_id, sources)?;

        {
            let mut processes = self.processes.lock().await;
            match processes.get(plugin_id).map(|process| process.is_alive()) {
                Some(true) => return Ok(workspace),
                // Its child exited or its stdout desynchronised. Left in the
                // map it would answer `contains_key` forever while every
                // request failed, and only an explicit shutdown would clear it.
                Some(false) => {
                    processes.remove(plugin_id);
                }
                None => {}
            }
        }

        let process = LspProcess::spawn(
            &exe,
            &workspace,
            Arc::clone(&self.client),
            plugin_id.to_string(),
        )
        .await?;

        if let Err(error) = initialise(&process, &workspace).await {
            process.shutdown().await;
            return Err(error);
        }

        self.processes
            .lock()
            .await
            .insert(plugin_id.to_string(), Arc::new(process));
        Ok(workspace)
    }
}

#[async_trait::async_trait]
impl LspService for ServerLspService {
    fn status(&self) -> TierStatus {
        self.tier.status()
    }

    /// Until a client is attached the frames a language server sends
    /// unprompted are emitted into a dropped channel and discarded.
    fn attach_client(&self, emitter: Emitter) {
        *self.client.lock().unwrap() = Some(emitter);
    }

    /// The uri returned is the same string `initialise` handed the language
    /// server as its `rootUri`, so a client building document uris under it
    /// names the very files the server indexed.
    async fn open_session(
        &self,
        plugin_id: &str,
        sources: &BTreeMap<String, String>,
    ) -> Result<String, String> {
        let workspace = self.ensure_ready(plugin_id, sources).await?;
        file_uri(&workspace)
    }

    async fn request(&self, plugin_id: &str, frame: Value) -> Result<Value, String> {
        self.process(plugin_id).await?.request(frame).await
    }

    async fn notify(&self, plugin_id: &str, frame: Value) -> Result<(), String> {
        self.process(plugin_id).await?.notify(frame).await
    }

    async fn shutdown(&self, plugin_id: &str) {
        let Some(process) = self.processes.lock().await.remove(plugin_id) else {
            return;
        };
        match Arc::try_unwrap(process) {
            Ok(process) => process.shutdown().await,
            // A call is still in flight; dropping the last handle kills the
            // child and aborts its tasks once that call returns.
            Err(shared) => drop(shared),
        }
    }
}

/// The LSP handshake. Dynamic registration and `workspace/configuration` are
/// declined because nothing here answers a server-initiated request — the
/// workspace's `.luarc.json` carries the configuration instead.
async fn initialise(process: &LspProcess, workspace: &Path) -> Result<(), String> {
    let root_uri = file_uri(workspace)?;
    let name = workspace
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "plugin".to_string());

    process
        .request(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": root_uri,
                "workspaceFolders": [{ "uri": root_uri, "name": name }],
                "capabilities": {
                    "textDocument": {
                        "synchronization": { "dynamicRegistration": false },
                        "publishDiagnostics": { "relatedInformation": true },
                        "hover": { "contentFormat": ["markdown", "plaintext"] },
                        "completion": { "completionItem": { "snippetSupport": false } },
                        "signatureHelp": {},
                        "definition": {},
                        "references": {},
                        "rename": { "prepareSupport": false },
                    },
                    "workspace": { "workspaceFolders": false, "configuration": false },
                },
            },
        }))
        .await?;

    process
        .notify(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {},
        }))
        .await
}

/// Percent-encodes only what a `file:` URI cannot carry literally; `/`, `:`
/// and the unreserved characters stay as they are so the result still reads
/// as a path.
const URI_PATH: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');

fn file_uri(path: &Path) -> Result<String, String> {
    let text = path
        .to_str()
        .ok_or_else(|| format!("{} is not valid UTF-8", path.display()))?;
    let slashed = text.replace('\\', "/");
    let bare = slashed.strip_prefix("//?/").unwrap_or(&slashed);
    let encoded = percent_encoding::utf8_percent_encode(bare, URI_PATH).to_string();
    if encoded.starts_with('/') {
        Ok(format!("file://{encoded}"))
    } else {
        Ok(format!("file:///{encoded}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::services::lsp_process::emit_to_client;

    fn service(dir: &Path) -> Arc<ServerLspService> {
        Arc::new(ServerLspService::new(
            dir.join("lua-language-server"),
            dir.join("plugin-workspaces"),
        ))
    }

    fn sources() -> BTreeMap<String, String> {
        BTreeMap::from([("main.lua".to_string(), "return {}".to_string())])
    }

    #[tokio::test]
    async fn a_request_for_a_plugin_that_is_not_open_says_so_instead_of_hanging() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let service = service(dir.path());
        let error = service
            .request(
                "user.demo",
                serde_json::json!({ "method": "textDocument/hover" }),
            )
            .await
            .expect_err("no process is running");
        assert!(error.contains("user.demo"), "{error}");
    }

    fn notification_frame(text: &str) -> Value {
        serde_json::from_str(text).expect("valid json")
    }

    #[tokio::test]
    async fn a_running_child_emits_to_whichever_client_attached_most_recently() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let service = service(dir.path());
        // Exactly what `LspProcess::spawn` hands a child's reader task, taken
        // once and never taken again — a page reload replaces the connection
        // without shutting the language server down.
        let held_since_spawn = Arc::clone(&service.client);

        let (first, mut first_frames) = Emitter::test_channel();
        service.attach_client(first);
        emit_to_client(
            &held_since_spawn,
            psp_app::messages::MessageType::LspNotification,
            &serde_json::json!({ "plugin_id": "user.demo" }),
        );
        let frame = notification_frame(
            &first_frames
                .try_recv()
                .expect("a language server's diagnostics must reach the attached client"),
        );
        assert_eq!(frame["type"], "lsp_notification");
        assert_eq!(frame["data"]["plugin_id"], "user.demo");

        let (second, mut second_frames) = Emitter::test_channel();
        service.attach_client(second);
        emit_to_client(
            &held_since_spawn,
            psp_app::messages::MessageType::LspNotification,
            &serde_json::json!({ "plugin_id": "user.demo" }),
        );

        let frame = notification_frame(&second_frames.try_recv().expect(
            "a reloaded page is a new connection; the child that is still up must follow it \
             rather than emit into the socket that went away",
        ));
        assert_eq!(frame["data"]["plugin_id"], "user.demo");
        assert!(
            first_frames.try_recv().is_err(),
            "the connection that was replaced must stop receiving frames"
        );
    }

    #[tokio::test]
    async fn shutting_down_a_plugin_that_was_never_open_is_a_no_op() {
        let dir = tempfile::tempdir().expect("a temp dir");
        service(dir.path()).shutdown("user.demo").await;
    }

    #[tokio::test]
    async fn the_tier_reports_starting_while_an_acquisition_is_in_flight() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let service = service(dir.path());

        if language_server::release_for_host().is_none() {
            assert!(
                matches!(service.status(), TierStatus::Unavailable { .. }),
                "a host off the release matrix can never reach the full tier"
            );
            return;
        }

        assert!(
            matches!(service.status(), TierStatus::Available),
            "the full tier is on offer before anything has been asked of it"
        );

        // Held so `ensure_ready` parks on the acquisition step, which is where
        // a real download would sit for as long as it takes.
        let acquiring = service.acquisition.lock().await;
        let running = tokio::spawn({
            let service = Arc::clone(&service);
            async move { service.ensure_ready("user.demo", &sources()).await }
        });

        let mut observed = false;
        for _ in 0..64 {
            if matches!(service.status(), TierStatus::Starting) {
                observed = true;
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            observed,
            "the tier must report Starting between the request to open a plugin and the \
             language server being up, not jump between terminal states"
        );

        running.abort();
        drop(acquiring);
        let _ = running.await;
    }

    #[test]
    fn a_file_uri_survives_a_windows_drive_letter_and_a_space() {
        let uri = file_uri(Path::new(r"C:\Users\a b\ws")).expect("a uri");
        assert_eq!(uri, "file:///C:/Users/a%20b/ws");
    }

    #[test]
    fn a_verbatim_prefix_is_stripped_before_the_uri_is_built() {
        let uri = file_uri(Path::new(r"\\?\C:\ws")).expect("a uri");
        assert_eq!(uri, "file:///C:/ws");
    }

    #[test]
    fn a_unix_style_absolute_path_keeps_its_single_leading_slash() {
        let uri = file_uri(Path::new("/home/a/ws")).expect("a uri");
        assert_eq!(uri, "file:///home/a/ws");
    }
}
