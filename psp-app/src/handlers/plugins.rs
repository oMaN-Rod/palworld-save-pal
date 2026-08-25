use std::collections::BTreeMap;
use std::sync::Arc;

use psp_plugin::manifest::{Capability, CommandDef, Manifest, Origin};
use psp_plugin::runtime::{run_command, RunOutcome, RunRequest, RunServices};
use psp_plugin::sandbox::{Cancel, Limits};
use psp_plugin::status::RunStatus;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::plugin_registry::BundledPlugin;
use crate::AppState;

const MAX_ZIP_ENTRIES: usize = 64;
const MAX_ZIP_FILE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, serde::Serialize)]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub enabled: bool,
    pub bundled: bool,
    pub commands: Vec<CommandDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn origin_of(bundled: bool) -> Origin {
    if bundled {
        Origin::Bundled
    } else {
        Origin::User
    }
}

fn parse_granted(raw: &str) -> Vec<Capability> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn summarize(row: &psp_db::plugins::PluginRow) -> PluginSummary {
    match Manifest::parse(&row.manifest, origin_of(row.bundled)) {
        Ok(manifest) => PluginSummary {
            id: row.id.clone(),
            name: manifest.name,
            version: manifest.version,
            author: manifest.author,
            enabled: row.enabled,
            bundled: row.bundled,
            commands: manifest.commands,
            error: None,
        },
        Err(parse_error) => PluginSummary {
            id: row.id.clone(),
            name: row.id.clone(),
            version: String::new(),
            author: None,
            enabled: row.enabled,
            bundled: row.bundled,
            commands: Vec::new(),
            error: Some(parse_error.to_string()),
        },
    }
}

pub async fn handle_list_plugins(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let rows = psp_db::plugins::get_all(&*ctx.app.driver).await?;
    let summaries: Vec<PluginSummary> = rows.iter().map(summarize).collect();
    ctx.emitter.emit(MessageType::ListPlugins, &summaries);
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct PluginIdData {
    pub id: String,
}

pub async fn handle_get_plugin(
    data: PluginIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(row) = psp_db::plugins::get(&*ctx.app.driver, &data.id).await? else {
        ctx.emitter.emit(
            MessageType::Error,
            &format!("plugin {} not found", data.id),
        );
        return Ok(());
    };

    let manifest = match Manifest::parse(&row.manifest, origin_of(row.bundled)) {
        Ok(manifest) => manifest,
        Err(parse_error) => {
            ctx.emitter.emit(
                MessageType::Error,
                &format!("plugin {} has an invalid manifest: {parse_error}", data.id),
            );
            return Ok(());
        }
    };
    let sources: BTreeMap<String, String> =
        serde_json::from_str(&row.sources).unwrap_or_default();
    let granted = parse_granted(&row.granted_capabilities);

    ctx.emitter.emit(
        MessageType::GetPlugin,
        &serde_json::json!({
            "id": row.id,
            "manifest": manifest,
            "sources": sources,
            "enabled": row.enabled,
            "bundled": row.bundled,
            "granted_capabilities": granted,
        }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct InstallPluginData {
    pub filename: String,
    /// Base64-encoded; decoded whole, bounded only by the transport's frame size cap.
    pub content: String,
}

fn is_ident_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn defines_global_function(source: &str, name: &str) -> bool {
    let bytes = source.as_bytes();
    let mut search_from = 0usize;
    while let Some(relative) = source[search_from..].find("function") {
        let start = search_from + relative;
        let before_ok = start == 0 || !is_ident_char(bytes[start - 1]);
        let mut cursor = start + "function".len();
        if before_ok {
            while cursor < bytes.len() && (bytes[cursor] as char).is_whitespace() {
                cursor += 1;
            }
            if source[cursor..].starts_with(name) {
                let name_end = cursor + name.len();
                let end_ok = name_end >= bytes.len() || !is_ident_char(bytes[name_end]);
                if end_ok {
                    let mut paren = name_end;
                    while paren < bytes.len() && (bytes[paren] as char).is_whitespace() {
                        paren += 1;
                    }
                    if bytes.get(paren) == Some(&b'(') {
                        return true;
                    }
                }
            }
        }
        search_from = start + "function".len();
    }
    false
}

fn slugify_id(stem: &str) -> String {
    let mut out = String::new();
    let mut previous_was_sep = true;
    for ch in stem.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            previous_was_sep = false;
        } else if !previous_was_sep {
            out.push('-');
            previous_was_sep = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > 64 {
        out.truncate(64);
        while out.ends_with('-') {
            out.pop();
        }
    }
    if out.is_empty() {
        out.push_str("plugin");
    }
    out
}

fn install_error(ctx: &HandlerCtx<'_>, message: impl Into<String>) -> Result<(), HandlerError> {
    ctx.emitter.emit(MessageType::Error, &message.into());
    Ok(())
}

/// Windows resolves a reserved device name (`CON`, `NUL`, `COM1`, ...) per
/// path component, case-insensitively, against the stem before the first
/// `.` — regardless of extension or directory prefix, so `CON`, `CON.lua`
/// and `CON.tar.gz` all resolve to the same device rather than a file.
fn is_reserved_device_name(segment: &str) -> bool {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = segment.split('.').next().unwrap_or(segment);
    RESERVED.iter().any(|reserved| reserved.eq_ignore_ascii_case(stem))
}

/// Rejects names that could escape the archive on extraction, or that carry a
/// non-ASCII character a filesystem could fold to an ASCII look-alike.
fn is_safe_zip_entry_name(name: &str) -> bool {
    if name.is_empty() || name.contains("..") {
        return false;
    }
    if name.starts_with('/') || name.starts_with('\\') {
        return false;
    }
    if !name.is_ascii() {
        return false;
    }
    if name.contains(':') {
        return false;
    }
    // Inert today (matches neither manifest.json nor *.lua) but reject anyway
    // rather than rely on that staying true.
    if name.chars().all(|c| c == '.') {
        return false;
    }
    if is_reserved_device_name(name) {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Sanity bounds on the stored key, not a guarantee about any assembled
/// on-disk path: 240 keeps a single segment under NTFS's 255-character
/// component limit and stops an unbounded string reaching downstream
/// consumers; 16 segments bounds the cost of walking the path. A future
/// on-disk writer must still budget against its own resolved root length.
const MAX_SOURCE_PATH_LEN: usize = 240;
const MAX_SOURCE_PATH_SEGMENTS: usize = 16;

/// Rejects a plugin source path that is not a plain, relative, forward-slash
/// `.lua` path: this key is later written to disk as a real filesystem path,
/// so anything that could escape the plugin's own directory, resolve to a
/// Windows device, or otherwise misbehave on write must be refused here
/// rather than trusted from a client-controlled request.
fn is_safe_source_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return false;
    }
    if path.len() > MAX_SOURCE_PATH_LEN {
        return false;
    }
    if !path.is_ascii() {
        return false;
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    if path.chars().any(|c| c.is_control()) {
        return false;
    }
    if !path.ends_with(".lua") {
        return false;
    }
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() > MAX_SOURCE_PATH_SEGMENTS {
        return false;
    }
    segments.iter().all(|segment| {
        !segment.is_empty()
            && *segment != "."
            && *segment != ".."
            && !segment.ends_with('.')
            && !segment.ends_with(' ')
            && !is_reserved_device_name(segment)
    })
}

struct ParsedZip {
    manifest_json: String,
    sources: BTreeMap<String, String>,
}

/// `zip::ZipArchive` silently collapses entries that share a name (keeps the
/// later one) before `len()` ever sees the duplicate; comparing this raw EOCD
/// count against `archive.len()` is the only way to notice that happened.
/// `None` means "cannot tell", not "no duplicates".
fn raw_declared_entry_count(bytes: &[u8]) -> Option<u16> {
    const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    const EOCD_MIN_LEN: usize = 22;
    if bytes.len() < EOCD_MIN_LEN {
        return None;
    }
    // The EOCD comment field can be up to 65535 bytes, so the signature isn't
    // at a fixed offset from the end.
    let search_start = bytes.len().saturating_sub(EOCD_MIN_LEN + 0xFFFF);
    let signature_pos = bytes[search_start..].windows(4).rposition(|w| w == EOCD_SIGNATURE)?;
    let eocd_start = search_start + signature_pos;
    if bytes.len() < eocd_start + EOCD_MIN_LEN {
        return None;
    }
    let total_entries = u16::from_le_bytes([bytes[eocd_start + 10], bytes[eocd_start + 11]]);
    if total_entries == 0xFFFF {
        return None;
    }
    Some(total_entries)
}

fn read_zip_plugin(bytes: &[u8]) -> Result<ParsedZip, String> {
    use std::io::Read;

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("could not read zip archive: {e}"))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(format!(
            "zip archive has too many entries (max {MAX_ZIP_ENTRIES})"
        ));
    }
    if let Some(declared) = raw_declared_entry_count(bytes) {
        if declared as usize != archive.len() {
            return Err("zip archive has duplicate entry names".to_string());
        }
    }

    let mut manifest_json: Option<String> = None;
    let mut sources = BTreeMap::new();

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("could not read zip entry: {e}"))?;
        // Validated before the directory check so a directory entry can't skip
        // the same attacker-controlled-name gate every other entry clears.
        let name = entry.name().to_string();
        if !is_safe_zip_entry_name(&name) {
            return Err(format!("zip entry {name:?} has an unsafe path"));
        }
        if entry.is_dir() {
            continue;
        }

        let mut limited = (&mut entry).take(MAX_ZIP_FILE_BYTES + 1);
        let mut contents = Vec::new();
        limited
            .read_to_end(&mut contents)
            .map_err(|e| format!("could not read zip entry {name:?}: {e}"))?;
        if contents.len() as u64 > MAX_ZIP_FILE_BYTES {
            return Err(format!("zip entry {name:?} exceeds the 1 MiB limit"));
        }
        let text = String::from_utf8(contents)
            .map_err(|_| format!("zip entry {name:?} is not valid UTF-8"))?;

        if name.eq_ignore_ascii_case("manifest.json") {
            manifest_json = Some(text);
        } else if name.to_ascii_lowercase().ends_with(".lua") {
            sources.insert(name, text);
        }
    }

    let manifest_json =
        manifest_json.ok_or_else(|| "zip archive is missing manifest.json".to_string())?;
    Ok(ParsedZip { manifest_json, sources })
}

pub async fn handle_install_plugin(
    data: InstallPluginData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    use base64::Engine as _;

    let bytes = match base64::engine::general_purpose::STANDARD.decode(&data.content) {
        Ok(bytes) => bytes,
        Err(decode_error) => {
            return install_error(ctx, format!("invalid base64: {decode_error}"));
        }
    };

    let lower_name = data.filename.to_ascii_lowercase();
    let (manifest_json, sources) = if lower_name.ends_with(".zip") {
        match read_zip_plugin(&bytes) {
            Ok(parsed) => (parsed.manifest_json, parsed.sources),
            Err(message) => return install_error(ctx, message),
        }
    } else if lower_name.ends_with(".lua") {
        let source = match String::from_utf8(bytes) {
            Ok(source) => source,
            Err(_) => return install_error(ctx, "the .lua file is not valid UTF-8"),
        };
        if !defines_global_function(&source, "main") {
            return install_error(
                ctx,
                "a bare .lua install must define a top-level `function main()`",
            );
        }
        let stem = lower_name
            .rfind(".lua")
            .map(|pos| &data.filename[..pos])
            .unwrap_or(&data.filename);
        let id = slugify_id(stem);
        let manifest = serde_json::json!({
            "id": id,
            "api_version": 1,
            "name": id,
            "version": "1.0.0",
            "entry": "main.lua",
            "capabilities": [],
            "commands": [{ "id": "main", "title": "Main" }],
        });
        let manifest_json = manifest.to_string();
        let mut sources = BTreeMap::new();
        sources.insert("main.lua".to_string(), source);
        (manifest_json, sources)
    } else {
        return install_error(ctx, "install_plugin accepts only a .lua file or a .zip archive");
    };

    let manifest = match Manifest::parse(&manifest_json, Origin::User) {
        Ok(manifest) => manifest,
        Err(manifest_error) => return install_error(ctx, manifest_error.to_string()),
    };
    if !sources.contains_key(&manifest.entry) {
        return install_error(
            ctx,
            format!("manifest entry {:?} has no matching source file", manifest.entry),
        );
    }
    if let Some(existing) = psp_db::plugins::get(&*ctx.app.driver, &manifest.id).await? {
        if existing.bundled {
            return install_error(
                ctx,
                format!("plugin {:?} is a bundled plugin and cannot be overwritten by an install", manifest.id),
            );
        }
    }

    let sources_json = serde_json::to_string(&sources)?;
    let granted_json = serde_json::to_string(&manifest.capabilities)?;
    let manifest_canonical = serde_json::to_string(&manifest)?;

    let row = psp_db::plugins::upsert(
        &*ctx.app.driver,
        &psp_db::plugins::NewPlugin {
            id: &manifest.id,
            manifest: &manifest_canonical,
            sources: &sources_json,
            granted_capabilities: &granted_json,
            bundled: false,
        },
    )
    .await?;

    ctx.emitter.emit(MessageType::InstallPlugin, &summarize(&row));
    Ok(())
}

pub async fn handle_uninstall_plugin(
    data: PluginIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(row) = psp_db::plugins::get(&*ctx.app.driver, &data.id).await? else {
        return install_error(ctx, format!("plugin {} not found", data.id));
    };
    if row.bundled {
        return install_error(ctx, "a bundled plugin cannot be uninstalled");
    }
    psp_db::plugins::remove(&*ctx.app.driver, &data.id).await?;
    ctx.emitter.emit(
        MessageType::UninstallPlugin,
        &serde_json::json!({ "id": data.id }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct SetPluginEnabledData {
    pub id: String,
    pub enabled: bool,
}

pub async fn handle_set_plugin_enabled(
    data: SetPluginEnabledData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let found = psp_db::plugins::set_enabled(&*ctx.app.driver, &data.id, data.enabled).await?;
    if !found {
        return install_error(ctx, format!("plugin {} not found", data.id));
    }
    let rows = psp_db::plugins::get_all(&*ctx.app.driver).await?;
    let summaries: Vec<PluginSummary> = rows.iter().map(summarize).collect();
    ctx.emitter.emit(MessageType::ListPlugins, &summaries);
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct RunPluginCommandData {
    pub plugin_id: String,
    pub command_id: String,
    #[serde(default)]
    pub args: serde_json::Value,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(serde::Serialize)]
struct RunResultLog {
    level: &'static str,
    message: String,
}

fn log_level_wire(level: psp_plugin::context::LogLevel) -> &'static str {
    use psp_plugin::context::LogLevel;
    match level {
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    }
}

fn emit_run_result(
    ctx: &HandlerCtx<'_>,
    run_id: uuid::Uuid,
    status: &RunStatus,
    summary: Option<&str>,
    counts: &BTreeMap<String, i64>,
    result: Option<&serde_json::Value>,
    log: &[psp_plugin::context::LogLine],
) {
    let (status_wire, message) = match status {
        RunStatus::Error(text) => ("error", Some(text.clone())),
        other => (other.as_wire(), None),
    };
    let log: Vec<RunResultLog> = log
        .iter()
        .map(|line| RunResultLog {
            level: log_level_wire(line.level),
            message: line.message.clone(),
        })
        .collect();
    ctx.emitter.emit(
        MessageType::PluginRunResult,
        &serde_json::json!({
            "run_id": run_id,
            "status": status_wire,
            "message": message,
            "summary": summary,
            "counts": counts,
            "result": result,
            "log": log,
        }),
    );
}

/// Releases a run's `Cancel` handle from the registry on drop, including on
/// an unwind, so a panicking run cannot leak the entry.
struct RunGuard<'a> {
    app: &'a AppState,
    run_id: uuid::Uuid,
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        self.app.plugins.finish_run(self.run_id);
    }
}

fn emit_refused_run(ctx: &HandlerCtx<'_>, message: impl Into<String>) {
    let run_id = uuid::Uuid::new_v4();
    emit_run_result(
        ctx,
        run_id,
        &RunStatus::Error(message.into()),
        None,
        &BTreeMap::new(),
        None,
        &[],
    );
}

struct RunOverrides<'a> {
    manifest: Option<&'a str>,
    sources: &'a BTreeMap<String, String>,
}

/// `Draft` carries caller-supplied sources, so it is refused for a bundled
/// row: `save.raw` and similar capabilities exist only for code shipped
/// inside the binary.
enum RunMode<'a> {
    Installed,
    Draft(RunOverrides<'a>),
}

/// The one place a run's capability set is computed, so the row's
/// `granted_capabilities` ceiling cannot be bypassed by only one entry point.
async fn run_plugin(
    ctx: &mut HandlerCtx<'_>,
    plugin_id: &str,
    command_id: &str,
    args: &serde_json::Value,
    dry_run: bool,
    mode: RunMode<'_>,
) -> Result<(), HandlerError> {
    let Some(row) = psp_db::plugins::get(&*ctx.app.driver, plugin_id).await? else {
        emit_refused_run(ctx, format!("plugin {plugin_id} not found"));
        return Ok(());
    };
    let overrides = match &mode {
        RunMode::Installed => None,
        RunMode::Draft(overrides) => Some(overrides),
    };
    if overrides.is_some() && row.bundled {
        emit_refused_run(
            ctx,
            format!(
                "plugin {plugin_id} is bundled: a draft run would execute edited code under a bundled plugin's privileges, so drafts are refused here"
            ),
        );
        return Ok(());
    }
    if overrides.is_none() && !row.enabled {
        emit_refused_run(ctx, format!("plugin {plugin_id} is disabled"));
        return Ok(());
    }

    let manifest_json = overrides.and_then(|o| o.manifest).unwrap_or(&row.manifest);
    let manifest = match Manifest::parse(manifest_json, origin_of(row.bundled)) {
        Ok(manifest) => manifest,
        Err(manifest_error) => {
            emit_refused_run(ctx, manifest_error.to_string());
            return Ok(());
        }
    };

    let row_granted = parse_granted(&row.granted_capabilities);
    let granted: Vec<Capability> = manifest
        .capabilities
        .iter()
        .copied()
        .filter(|capability| row_granted.contains(capability))
        .collect();

    let stored_sources: BTreeMap<String, String> =
        serde_json::from_str(&row.sources).unwrap_or_default();
    let sources = overrides.map(|o| o.sources).unwrap_or(&stored_sources);

    let session = match ctx.session.save_mut() {
        Ok(session) => session,
        Err(_) => {
            emit_refused_run(ctx, "no save file loaded");
            return Ok(());
        }
    };

    let storage = psp_db::plugins::storage_get_all(&*ctx.app.driver, plugin_id).await?;

    let run_id = uuid::Uuid::new_v4();
    let cancel = Cancel::new();
    ctx.app.plugins.register_run(run_id, cancel.clone());
    let _run_guard = RunGuard { app: ctx.app.as_ref(), run_id };

    let outcome: RunOutcome = run_command(
        RunRequest {
            manifest: &manifest,
            sources,
            command_id,
            args,
            dry_run,
            granted: &granted,
        },
        RunServices {
            session,
            game_data: &ctx.app.game_data,
            progress: Some(&ctx.emitter.progress_sink()),
            storage: &storage,
            confirm: None,
            limits: Limits::default(),
            cancel,
        },
    );

    if outcome.status.is_ok() && !outcome.storage_writes.is_empty() {
        psp_db::plugins::storage_put_many(&*ctx.app.driver, plugin_id, &outcome.storage_writes)
            .await?;
    }

    emit_run_result(
        ctx,
        run_id,
        &outcome.status,
        outcome.summary.as_deref(),
        &outcome.counts,
        outcome.result.as_ref(),
        &outcome.log,
    );
    Ok(())
}

pub async fn handle_run_plugin_command(
    data: RunPluginCommandData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    run_plugin(
        ctx,
        &data.plugin_id,
        &data.command_id,
        &data.args,
        data.dry_run,
        RunMode::Installed,
    )
    .await
}

#[derive(Debug, serde::Deserialize)]
pub struct RunPluginDraftData {
    pub plugin_id: String,
    pub command_id: String,
    #[serde(default)]
    pub args: serde_json::Value,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub sources: BTreeMap<String, String>,
    #[serde(default)]
    pub manifest: Option<String>,
}

pub async fn handle_run_plugin_draft(
    data: RunPluginDraftData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    run_plugin(
        ctx,
        &data.plugin_id,
        &data.command_id,
        &data.args,
        data.dry_run,
        RunMode::Draft(RunOverrides {
            manifest: data.manifest.as_deref(),
            sources: &data.sources,
        }),
    )
    .await
}

#[derive(Debug, serde::Deserialize)]
pub struct CancelPluginRunData {
    pub run_id: uuid::Uuid,
}

pub async fn handle_cancel_plugin_run(
    data: CancelPluginRunData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    ctx.app.plugins.cancel_run(data.run_id);
    Ok(())
}

fn bundled_sources(plugin: &BundledPlugin) -> BTreeMap<String, String> {
    plugin
        .sources
        .iter()
        .map(|(name, content)| (name.to_string(), content.to_string()))
        .collect()
}

/// A bundled plugin's `granted_capabilities` on first seed is its manifest's
/// full capability list — bundled plugins are first-party.
pub async fn seed_bundled_plugins(app: &Arc<AppState>) -> Result<(), HandlerError> {
    for plugin in app.plugins.bundled() {
        let manifest = Manifest::parse(plugin.manifest, Origin::Bundled)
            .map_err(|e| HandlerError::Other(format!("bundled plugin {}: {e}", plugin.id)))?;
        let sources = bundled_sources(plugin);
        let sources_json = serde_json::to_string(&sources)?;
        let granted_json = serde_json::to_string(&manifest.capabilities)?;
        psp_db::plugins::seed_bundled(
            &*app.driver,
            &psp_db::plugins::NewPlugin {
                id: plugin.id,
                manifest: plugin.manifest,
                sources: &sources_json,
                granted_capabilities: &granted_json,
                bundled: true,
            },
        )
        .await?;
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct CheckPluginSyntaxData {
    pub source: String,
}

pub async fn handle_check_plugin_syntax(
    data: CheckPluginSyntaxData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let error = psp_plugin::syntax::check(&data.source);
    ctx.emitter
        .emit(MessageType::CheckPluginSyntax, &serde_json::json!({ "error": error }));
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct CheckPluginManifestData {
    /// Names the row whose origin the manifest is judged under; unknown/absent
    /// falls back to `Origin::User`, which refuses more.
    #[serde(default)]
    pub id: Option<String>,
    pub manifest: String,
}

pub async fn handle_check_plugin_manifest(
    data: CheckPluginManifestData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let origin = match &data.id {
        Some(id) => match psp_db::plugins::get(&*ctx.app.driver, id).await? {
            Some(row) => origin_of(row.bundled),
            None => Origin::User,
        },
        None => Origin::User,
    };
    let error = Manifest::parse(&data.manifest, origin)
        .err()
        .map(|parse_error| parse_error.to_string());
    ctx.emitter
        .emit(MessageType::CheckPluginManifest, &serde_json::json!({ "error": error }));
    Ok(())
}

pub async fn handle_get_api_definition(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    ctx.emitter
        .emit(MessageType::GetApiDefinition, &psp_plugin::api_definition());
    Ok(())
}

const SCAFFOLD_COMMAND: &str = "run";

fn escape_lua_string(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn scaffold_source(name: &str) -> String {
    let name = escape_lua_string(name);
    format!(
        "function {SCAFFOLD_COMMAND}()\n  log.info(\"{name} ran\")\n  return {{ summary = \"{name} did nothing yet\" }}\nend\n"
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct CreatePluginData {
    pub id: String,
    pub name: String,
}

/// Answers under `create_plugin`, not `MessageType::Error` — the frontend's
/// request/response pairing depends on the reply using the request's own
/// message type.
fn create_refused(ctx: &HandlerCtx<'_>, message: impl Into<String>) -> Result<(), HandlerError> {
    ctx.emitter
        .emit(MessageType::CreatePlugin, &serde_json::json!({ "error": message.into() }));
    Ok(())
}

pub async fn handle_create_plugin(
    data: CreatePluginData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if psp_db::plugins::get(&*ctx.app.driver, &data.id).await?.is_some() {
        return create_refused(ctx, format!("plugin {:?} already exists", data.id));
    }

    let display_name = if data.name.trim().is_empty() { data.id.as_str() } else { data.name.as_str() };
    let manifest_json = serde_json::json!({
        "id": data.id,
        "api_version": 1,
        "name": display_name,
        "version": "0.1.0",
        "entry": "main.lua",
        "capabilities": ["log"],
        "commands": [{ "id": SCAFFOLD_COMMAND, "title": "Run" }],
    })
    .to_string();

    let manifest = match Manifest::parse(&manifest_json, Origin::User) {
        Ok(manifest) => manifest,
        Err(manifest_error) => return create_refused(ctx, manifest_error.to_string()),
    };

    let mut sources = BTreeMap::new();
    sources.insert("main.lua".to_string(), scaffold_source(display_name));

    let row = psp_db::plugins::upsert(
        &*ctx.app.driver,
        &psp_db::plugins::NewPlugin {
            id: &manifest.id,
            manifest: &serde_json::to_string(&manifest)?,
            sources: &serde_json::to_string(&sources)?,
            granted_capabilities: &serde_json::to_string(&manifest.capabilities)?,
            bundled: false,
        },
    )
    .await?;

    ctx.emitter.emit(MessageType::CreatePlugin, &summarize(&row));
    Ok(())
}

pub const MANIFEST_PATH: &str = "manifest.json";

#[derive(Debug, serde::Deserialize)]
pub struct SavePluginSourceData {
    pub id: String,
    pub path: String,
    pub source: String,
}

/// Answers under `save_plugin_source`, not `MessageType::Error`, for the same
/// request/response pairing reason as plugin creation.
fn save_refused(
    ctx: &HandlerCtx<'_>,
    data: &SavePluginSourceData,
    message: impl Into<String>,
) -> Result<(), HandlerError> {
    ctx.emitter.emit(
        MessageType::SavePluginSource,
        &serde_json::json!({ "id": data.id, "path": data.path, "error": message.into() }),
    );
    Ok(())
}

pub async fn handle_save_plugin_source(
    data: SavePluginSourceData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(row) = psp_db::plugins::get(&*ctx.app.driver, &data.id).await? else {
        return save_refused(ctx, &data, format!("plugin {} not found", data.id));
    };
    if row.bundled {
        return save_refused(
            ctx,
            &data,
            format!(
                "plugin {:?} is bundled: its sources are restored from the app on every launch, so an edit here would not survive a restart",
                data.id
            ),
        );
    }

    if data.path == MANIFEST_PATH {
        let manifest = match Manifest::parse(&data.source, origin_of(row.bundled)) {
            Ok(manifest) => manifest,
            Err(manifest_error) => return save_refused(ctx, &data, manifest_error.to_string()),
        };
        // The row's id keys its storage and its grant, so a mismatched
        // manifest id would leave `ctx.plugin_id` disagreeing with everything
        // the host keys off it.
        if manifest.id != data.id {
            return save_refused(
                ctx,
                &data,
                format!(
                    "this manifest declares id {:?}, but it is being saved into plugin {:?}",
                    manifest.id, data.id
                ),
            );
        }
        psp_db::plugins::set_manifest(
            &*ctx.app.driver,
            &data.id,
            &serde_json::to_string(&manifest)?,
        )
        .await?;
        psp_db::plugins::set_granted(
            &*ctx.app.driver,
            &data.id,
            &serde_json::to_string(&manifest.capabilities)?,
        )
        .await?;
    } else {
        if !is_safe_source_path(&data.path) {
            return save_refused(
                ctx,
                &data,
                format!("{:?} is not a valid plugin source path", data.path),
            );
        }
        let mut sources: BTreeMap<String, String> =
            serde_json::from_str(&row.sources).unwrap_or_default();
        sources.insert(data.path.clone(), data.source);
        psp_db::plugins::set_sources(
            &*ctx.app.driver,
            &data.id,
            &serde_json::to_string(&sources)?,
        )
        .await?;
    }

    ctx.emitter.emit(
        MessageType::SavePluginSource,
        &serde_json::json!({ "id": data.id, "path": data.path, "error": null }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct DeletePluginSourceData {
    pub id: String,
    pub path: String,
}

/// Answers under `delete_plugin_source`, not `MessageType::Error`, for the same
/// request/response pairing reason as plugin creation.
fn delete_refused(
    ctx: &HandlerCtx<'_>,
    data: &DeletePluginSourceData,
    message: impl Into<String>,
) -> Result<(), HandlerError> {
    ctx.emitter.emit(
        MessageType::DeletePluginSource,
        &serde_json::json!({ "id": data.id, "path": data.path, "error": message.into() }),
    );
    Ok(())
}

pub async fn handle_delete_plugin_source(
    data: DeletePluginSourceData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(row) = psp_db::plugins::get(&*ctx.app.driver, &data.id).await? else {
        return delete_refused(ctx, &data, format!("plugin {} not found", data.id));
    };
    if row.bundled {
        return delete_refused(
            ctx,
            &data,
            format!(
                "plugin {:?} is bundled: its sources are restored from the app on every launch",
                data.id
            ),
        );
    }
    if data.path == MANIFEST_PATH {
        return delete_refused(ctx, &data, "the manifest cannot be deleted");
    }

    let manifest = match Manifest::parse(&row.manifest, origin_of(row.bundled)) {
        Ok(manifest) => manifest,
        Err(manifest_error) => {
            return delete_refused(
                ctx,
                &data,
                format!("plugin {} has an invalid manifest: {manifest_error}", data.id),
            )
        }
    };
    if data.path == manifest.entry {
        return delete_refused(ctx, &data, "the entry source cannot be deleted");
    }

    let mut sources: BTreeMap<String, String> = serde_json::from_str(&row.sources)?;
    sources.remove(&data.path);
    psp_db::plugins::set_sources(&*ctx.app.driver, &data.id, &serde_json::to_string(&sources)?)
        .await?;

    ctx.emitter.emit(
        MessageType::DeletePluginSource,
        &serde_json::json!({ "id": data.id, "path": data.path }),
    );
    Ok(())
}

#[cfg(test)]
mod source_path_tests {
    use super::is_safe_source_path;

    #[test]
    fn accepts_a_nested_lua_path() {
        assert!(is_safe_source_path("lib/util.lua"));
    }

    #[test]
    fn accepts_a_bare_lua_path() {
        assert!(is_safe_source_path("main.lua"));
    }

    #[test]
    fn rejects_an_absolute_path() {
        assert!(!is_safe_source_path("/abs.lua"));
    }

    #[test]
    fn rejects_a_backslash() {
        assert!(!is_safe_source_path("lib\\util.lua"));
    }

    #[test]
    fn rejects_a_drive_letter_prefix() {
        assert!(!is_safe_source_path("C:/util.lua"));
        assert!(!is_safe_source_path("c:util.lua"));
    }

    #[test]
    fn rejects_a_dot_segment() {
        assert!(!is_safe_source_path("./util.lua"));
        assert!(!is_safe_source_path("lib/./util.lua"));
    }

    #[test]
    fn rejects_a_dot_dot_segment() {
        assert!(!is_safe_source_path("../util.lua"));
        assert!(!is_safe_source_path("lib/../../util.lua"));
    }

    #[test]
    fn rejects_an_empty_segment() {
        assert!(!is_safe_source_path("a//b.lua"));
    }

    #[test]
    fn rejects_a_trailing_dot_on_a_segment() {
        assert!(!is_safe_source_path("lib./util.lua"));
    }

    #[test]
    fn rejects_a_trailing_space_on_a_segment() {
        assert!(!is_safe_source_path("lib /util.lua"));
    }

    #[test]
    fn rejects_a_control_character() {
        assert!(!is_safe_source_path("lib/util\u{7}.lua"));
    }

    #[test]
    fn rejects_a_path_that_does_not_end_in_lua() {
        assert!(!is_safe_source_path("lib/util.txt"));
    }

    #[test]
    fn rejects_an_empty_path() {
        assert!(!is_safe_source_path(""));
    }

    #[test]
    fn rejects_every_reserved_device_name_bare_and_with_extension() {
        const RESERVED: &[&str] = &[
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        for reserved in RESERVED {
            let upper = format!("{reserved}.lua");
            let lower = format!("{}.lua", reserved.to_lowercase());
            let nested = format!("lib/{reserved}.lua");
            assert!(!is_safe_source_path(&upper), "{upper} must be refused");
            assert!(!is_safe_source_path(&lower), "{lower} must be refused");
            assert!(!is_safe_source_path(&nested), "{nested} must be refused");
        }
    }

    #[test]
    fn rejects_a_mixed_case_reserved_device_name() {
        assert!(!is_safe_source_path("CoN.lua"));
    }

    #[test]
    fn rejects_a_reserved_device_name_used_as_a_directory() {
        assert!(!is_safe_source_path("CON/x.lua"));
    }

    #[test]
    fn accepts_names_that_merely_resemble_a_reserved_device_name() {
        for near_miss in [
            "COM0.lua",
            "COM10.lua",
            "LPT0.lua",
            "CONS.lua",
            "CONSOLE.lua",
            "MYCON.lua",
            "CON2.lua",
            "AUXILIARY.lua",
            "NULL.lua",
            "PRNT.lua",
        ] {
            assert!(is_safe_source_path(near_miss), "{near_miss} must be accepted");
        }
    }

    #[test]
    fn rejects_non_ascii_look_alikes() {
        assert!(!is_safe_source_path("lib/\u{fc}til.lua"));
        assert!(!is_safe_source_path("lib\u{ff0f}util.lua"));
        assert!(!is_safe_source_path("lib/\u{202e}util.lua"));
        assert!(!is_safe_source_path("lib/util\u{a0}.lua"));
    }

    #[test]
    fn rejects_a_path_over_the_length_cap() {
        let long_name = "a".repeat(super::MAX_SOURCE_PATH_LEN);
        let path = format!("{long_name}.lua");
        assert!(path.len() > super::MAX_SOURCE_PATH_LEN);
        assert!(!is_safe_source_path(&path));
    }

    #[test]
    fn accepts_a_path_at_the_length_cap() {
        let filler = "a".repeat(super::MAX_SOURCE_PATH_LEN - ".lua".len());
        let path = format!("{filler}.lua");
        assert_eq!(path.len(), super::MAX_SOURCE_PATH_LEN);
        assert!(is_safe_source_path(&path));
    }

    #[test]
    fn rejects_a_path_over_the_segment_cap() {
        let path = format!("{}main.lua", "a/".repeat(super::MAX_SOURCE_PATH_SEGMENTS));
        assert!(!is_safe_source_path(&path));
    }

    #[test]
    fn accepts_a_path_at_the_segment_cap() {
        let path = format!("{}main.lua", "a/".repeat(super::MAX_SOURCE_PATH_SEGMENTS - 1));
        assert!(is_safe_source_path(&path));
    }
}

#[cfg(test)]
mod zip_entry_name_tests {
    use super::is_safe_zip_entry_name;

    #[test]
    fn accepts_a_plain_lua_name() {
        assert!(is_safe_zip_entry_name("main.lua"));
    }

    #[test]
    fn rejects_every_reserved_device_name_bare_and_with_extension() {
        const RESERVED: &[&str] = &[
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        for reserved in RESERVED {
            let bare = reserved.to_string();
            let upper = format!("{reserved}.lua");
            let lower = format!("{}.lua", reserved.to_lowercase());
            assert!(!is_safe_zip_entry_name(&bare), "{bare} must be refused");
            assert!(!is_safe_zip_entry_name(&upper), "{upper} must be refused");
            assert!(!is_safe_zip_entry_name(&lower), "{lower} must be refused");
        }
    }
}
