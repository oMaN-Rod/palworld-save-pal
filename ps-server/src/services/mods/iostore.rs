//! Downloads `retoc` on demand and converts a legacy `.pak` into IoStore
//! containers (`.utoc`/`.ucas`) for Game Pass targets.
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use super::frameworks::source::Progress;

pub const RETOC_VERSION: &str = "0.1.5";
pub const RETOC_ASSET: &str = "retoc_cli-x86_64-pc-windows-msvc.zip";
pub const RETOC_SHA256: &str = "cc036b06ad3bdcf7003690b00d82719980c374e48a95bf0654f9959148d263aa";

const MAX_TOOL_BYTES: u64 = 16 * 1024 * 1024;
const CONVERT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);
const STDERR_TAIL: usize = 2 * 1024;
const RETOC_EXE_ENTRY: &str = "retoc.exe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedPak {
    pub pak: PathBuf,
    pub utoc: PathBuf,
    pub ucas: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("Game Pass conversion is only supported on Windows")]
    UnsupportedPlatform,
    #[error("{0}")]
    ToolUnavailable(String),
    #[error("{0}")]
    ConversionFailed(String),
}

impl ConvertError {
    pub fn code(&self) -> &'static str {
        match self {
            ConvertError::UnsupportedPlatform => "unsupported_platform",
            ConvertError::ToolUnavailable(_) => "tool_unavailable",
            ConvertError::ConversionFailed(_) => "conversion_failed",
        }
    }
}

#[async_trait]
pub trait IoStoreConverter: Send + Sync {
    async fn convert(
        &self,
        pak: &Path,
        out_dir: &Path,
        progress: Progress<'_>,
    ) -> Result<ConvertedPak, ConvertError>;
}

pub struct RetocConverter {
    tools_dir: PathBuf,
    url: String,
    sha256: String,
    max_bytes: u64,
    http: reqwest::Client,
}

impl RetocConverter {
    pub fn github(app_root: &Path) -> Self {
        Self::new(
            app_root,
            &format!(
                "https://github.com/trumank/retoc/releases/download/v{RETOC_VERSION}/{RETOC_ASSET}"
            ),
            RETOC_SHA256,
        )
    }

    pub fn new(app_root: &Path, url: &str, sha256: &str) -> Self {
        Self {
            tools_dir: app_root.join("tools").join("retoc").join(RETOC_VERSION),
            url: url.to_string(),
            sha256: sha256.to_ascii_lowercase(),
            max_bytes: MAX_TOOL_BYTES,
            http: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("http client"),
        }
    }

    fn exe_path(&self) -> PathBuf {
        self.tools_dir.join(RETOC_EXE_ENTRY)
    }

    /// Downloads and verifies the tool when it is not already installed; a
    /// present `retoc.exe` short-circuits before any network call.
    async fn ensure_tool(&self, progress: Progress<'_>) -> Result<PathBuf, ConvertError> {
        let exe = self.exe_path();
        if exe.exists() {
            return Ok(exe);
        }
        tokio::fs::create_dir_all(&self.tools_dir)
            .await
            .map_err(|error| ConvertError::ToolUnavailable(error.to_string()))?;
        let unique = unique_suffix();
        let part = append_suffix(&exe, &format!("{unique}.part"));
        let result = self.install(&part, &exe, &unique, progress).await;
        let _ = tokio::fs::remove_file(&part).await;
        result?;
        Ok(exe)
    }

    async fn install(
        &self,
        part: &Path,
        exe: &Path,
        unique: &str,
        progress: Progress<'_>,
    ) -> Result<(), ConvertError> {
        let digest = self.download(part, progress).await?;
        if digest != self.sha256 {
            return Err(ConvertError::ToolUnavailable(
                "the downloaded retoc archive failed its checksum".to_string(),
            ));
        }
        let tmp = append_suffix(exe, &format!("{unique}.tmp"));
        if let Err(error) = extract_retoc_exe(part, &tmp) {
            let _ = std::fs::remove_file(&tmp);
            return Err(ConvertError::ToolUnavailable(error));
        }
        if let Err(error) = tokio::fs::rename(&tmp, exe).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(ConvertError::ToolUnavailable(error.to_string()));
        }
        Ok(())
    }

    async fn download(&self, part: &Path, progress: Progress<'_>) -> Result<String, ConvertError> {
        use tokio::io::AsyncWriteExt;
        let response = self
            .http
            .get(&self.url)
            .send()
            .await
            .map_err(|error| ConvertError::ToolUnavailable(error.to_string()))?;
        let mut response = response
            .error_for_status()
            .map_err(|error| ConvertError::ToolUnavailable(error.to_string()))?;
        let content_length = response.content_length();
        if let Some(len) = content_length {
            if len > self.max_bytes {
                return Err(ConvertError::ToolUnavailable(format!(
                    "the retoc download is larger than {} bytes",
                    self.max_bytes
                )));
            }
        }
        let mut file = tokio::fs::File::create(part)
            .await
            .map_err(|error| ConvertError::ToolUnavailable(error.to_string()))?;
        let mut hasher = Sha256::new();
        let mut total = 0u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| ConvertError::ToolUnavailable(error.to_string()))?
        {
            total += chunk.len() as u64;
            if total > self.max_bytes {
                return Err(ConvertError::ToolUnavailable(format!(
                    "the retoc download is larger than {} bytes",
                    self.max_bytes
                )));
            }
            hasher.update(&chunk);
            file.write_all(&chunk)
                .await
                .map_err(|error| ConvertError::ToolUnavailable(error.to_string()))?;
            progress(total, content_length);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

static UNIQUE_SUFFIX_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-download tag so two concurrent first-time conversions never write
/// through the same partial-download or extraction path.
fn unique_suffix() -> String {
    let counter = UNIQUE_SUFFIX_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{counter}", std::process::id())
}

/// Extracts only the `retoc.exe` entry, ignoring the path it was stored under
/// in the archive: `dest` is chosen by the caller, never by the zip.
fn extract_retoc_exe(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    let mut entry = archive
        .by_name(RETOC_EXE_ENTRY)
        .map_err(|_| format!("{RETOC_EXE_ENTRY} is not in the retoc archive"))?;
    let mut out = std::fs::File::create(dest).map_err(|error| error.to_string())?;
    let copied = std::io::copy(&mut (&mut entry).take(MAX_TOOL_BYTES), &mut out)
        .map_err(|error| error.to_string())?;
    if copied == MAX_TOOL_BYTES && entry.read(&mut [0u8; 1]).map_err(|error| error.to_string())? > 0
    {
        return Err(format!(
            "{RETOC_EXE_ENTRY} is larger than {MAX_TOOL_BYTES} bytes"
        ));
    }
    Ok(())
}

/// `<out_dir>/<pak's file stem>.<extension>` — the name `retoc` writes its
/// outputs under and the name `command_args` points it at.
fn named_output(pak: &Path, out_dir: &Path, extension: &str) -> PathBuf {
    let stem = pak.file_stem().unwrap_or(pak.as_os_str());
    let mut name = stem.to_os_string();
    name.push(".");
    name.push(extension);
    out_dir.join(name)
}

pub fn command_args(pak: &Path, out_dir: &Path) -> Vec<std::ffi::OsString> {
    vec![
        "to-zen".into(),
        "--version".into(),
        "UE5_1".into(),
        pak.as_os_str().to_os_string(),
        named_output(pak, out_dir, "utoc").into_os_string(),
    ]
}

fn stderr_tail(bytes: &[u8]) -> String {
    let start = bytes.len().saturating_sub(STDERR_TAIL);
    String::from_utf8_lossy(&bytes[start..]).trim().to_string()
}

async fn run_retoc(exe: &Path, pak: &Path, out_dir: &Path) -> Result<(), ConvertError> {
    tokio::fs::create_dir_all(out_dir)
        .await
        .map_err(|error| ConvertError::ConversionFailed(error.to_string()))?;
    let mut command = tokio::process::Command::new(exe);
    command
        .args(command_args(pak, out_dir))
        .kill_on_drop(true)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|error| ConvertError::ConversionFailed(error.to_string()))?;
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let reader_buf = stderr_buf.clone();
    let reader = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes).await;
        *reader_buf.lock().unwrap() = bytes;
    });
    let status = match tokio::time::timeout(CONVERT_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => return Err(ConvertError::ConversionFailed(error.to_string())),
        Err(_) => {
            let _ = child.start_kill();
            let _ = reader.await;
            let tail = stderr_tail(&stderr_buf.lock().unwrap());
            return Err(ConvertError::ConversionFailed(format!(
                "retoc timed out: {tail}"
            )));
        }
    };
    let _ = reader.await;
    if !status.success() {
        return Err(ConvertError::ConversionFailed(stderr_tail(
            &stderr_buf.lock().unwrap(),
        )));
    }
    Ok(())
}

#[async_trait]
impl IoStoreConverter for RetocConverter {
    async fn convert(
        &self,
        pak: &Path,
        out_dir: &Path,
        progress: Progress<'_>,
    ) -> Result<ConvertedPak, ConvertError> {
        if !cfg!(target_os = "windows") {
            return Err(ConvertError::UnsupportedPlatform);
        }
        let exe = self.ensure_tool(progress).await?;
        run_retoc(&exe, pak, out_dir).await?;
        let utoc = named_output(pak, out_dir, "utoc");
        let ucas = named_output(pak, out_dir, "ucas");
        let converted_pak = named_output(pak, out_dir, "pak");
        for path in [&utoc, &ucas, &converted_pak] {
            if !path.exists() {
                return Err(ConvertError::ConversionFailed(format!(
                    "{} was not produced by retoc",
                    path.display()
                )));
            }
        }
        Ok(ConvertedPak {
            pak: converted_pak,
            utoc,
            ucas,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn zip_bytes(name: &str, contents: &[u8]) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents).unwrap();
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn has_part_file(dir: &Path) -> bool {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .any(|entry| entry.path().extension().is_some_and(|ext| ext == "part"))
            })
            .unwrap_or(false)
    }

    async fn serve(body: Vec<u8>) -> (String, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_for_route = hits.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = axum::Router::new().route(
            "/retoc.zip",
            axum::routing::get(move || {
                let hits = hits_for_route.clone();
                let body = body.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    body
                }
            }),
        );
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        (format!("http://{addr}/retoc.zip"), hits)
    }

    /// A response with no `Content-Length`: axum's `Body::from_stream` sends
    /// it chunked, so only the streamed byte counter can catch an over-cap
    /// body here.
    async fn serve_streamed(total_len: usize) -> (String, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_for_route = hits.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let chunk_len = 4096usize;
        let chunk_count = total_len.div_ceil(chunk_len);
        let router = axum::Router::new().route(
            "/retoc.zip",
            axum::routing::get(move || {
                let hits = hits_for_route.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    let chunk = axum::body::Bytes::from(vec![0u8; chunk_len]);
                    let chunks: Vec<Result<axum::body::Bytes, std::io::Error>> =
                        std::iter::repeat_n(chunk, chunk_count).map(Ok).collect();
                    axum::body::Body::from_stream(futures::stream::iter(chunks))
                }
            }),
        );
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        (format!("http://{addr}/retoc.zip"), hits)
    }

    #[tokio::test]
    async fn ensure_tool_installs_a_verified_zip_and_leaves_no_part() {
        let zip = zip_bytes(RETOC_EXE_ENTRY, b"stub bytes");
        let sha256 = sha256_hex(&zip);
        let (url, hits) = serve(zip).await;
        let app_root = tempfile::tempdir().unwrap();
        let converter = RetocConverter::new(app_root.path(), &url, &sha256);
        let progress: Progress = &|_, _| {};

        let exe = converter.ensure_tool(progress).await.unwrap();

        assert_eq!(std::fs::read(&exe).unwrap(), b"stub bytes");
        assert!(!has_part_file(&converter.tools_dir));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn ensure_tool_rejects_a_wrong_checksum() {
        let zip = zip_bytes(RETOC_EXE_ENTRY, b"stub bytes");
        let (url, _hits) = serve(zip).await;
        let app_root = tempfile::tempdir().unwrap();
        let converter = RetocConverter::new(app_root.path(), &url, &"0".repeat(64));
        let progress: Progress = &|_, _| {};

        let error = converter.ensure_tool(progress).await.unwrap_err();

        assert!(matches!(error, ConvertError::ToolUnavailable(_)));
        assert!(!converter.exe_path().exists());
        assert!(!has_part_file(&converter.tools_dir));
    }

    #[tokio::test]
    async fn ensure_tool_rejects_a_zip_with_no_retoc_entry() {
        let zip = zip_bytes("other.txt", b"not retoc");
        let sha256 = sha256_hex(&zip);
        let (url, _hits) = serve(zip).await;
        let app_root = tempfile::tempdir().unwrap();
        let converter = RetocConverter::new(app_root.path(), &url, &sha256);
        let progress: Progress = &|_, _| {};

        let error = converter.ensure_tool(progress).await.unwrap_err();

        assert!(matches!(error, ConvertError::ToolUnavailable(_)));
        assert!(!converter.exe_path().exists());
        assert!(!has_part_file(&converter.tools_dir));
    }

    #[tokio::test]
    async fn ensure_tool_rejects_a_body_over_the_cap() {
        let body = vec![0u8; (MAX_TOOL_BYTES + 1) as usize];
        let (url, _hits) = serve(body).await;
        let app_root = tempfile::tempdir().unwrap();
        let converter = RetocConverter::new(app_root.path(), &url, &"0".repeat(64));
        let progress: Progress = &|_, _| {};

        let error = converter.ensure_tool(progress).await.unwrap_err();

        assert!(matches!(error, ConvertError::ToolUnavailable(_)));
        assert!(!converter.exe_path().exists());
        assert!(!has_part_file(&converter.tools_dir));
    }

    #[tokio::test]
    async fn ensure_tool_rejects_a_streamed_body_over_the_cap_with_no_content_length() {
        let (url, _hits) = serve_streamed((MAX_TOOL_BYTES + 1) as usize).await;
        let app_root = tempfile::tempdir().unwrap();
        let converter = RetocConverter::new(app_root.path(), &url, &"0".repeat(64));
        let progress: Progress = &|_, _| {};

        let error = converter.ensure_tool(progress).await.unwrap_err();

        assert!(matches!(error, ConvertError::ToolUnavailable(_)));
        assert!(!converter.exe_path().exists());
        assert!(!has_part_file(&converter.tools_dir));
    }

    #[tokio::test]
    async fn ensure_tool_makes_no_request_when_already_installed() {
        let (url, hits) = serve(Vec::new()).await;
        let app_root = tempfile::tempdir().unwrap();
        let converter = RetocConverter::new(app_root.path(), &url, &"0".repeat(64));
        std::fs::create_dir_all(converter.exe_path().parent().unwrap()).unwrap();
        std::fs::write(converter.exe_path(), b"already installed").unwrap();
        let progress: Progress = &|_, _| {};

        let exe = converter.ensure_tool(progress).await.unwrap();

        assert_eq!(std::fs::read(&exe).unwrap(), b"already installed");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn two_concurrent_first_installs_do_not_clobber_each_others_download() {
        let zip = zip_bytes(RETOC_EXE_ENTRY, b"stub bytes");
        let sha256 = sha256_hex(&zip);
        let (url, _hits) = serve(zip).await;
        let app_root = tempfile::tempdir().unwrap();
        let converter = RetocConverter::new(app_root.path(), &url, &sha256);
        let progress: Progress = &|_, _| {};

        let (first, second) = tokio::join!(
            converter.ensure_tool(progress),
            converter.ensure_tool(progress)
        );

        assert_eq!(std::fs::read(first.unwrap()).unwrap(), b"stub bytes");
        assert_eq!(std::fs::read(second.unwrap()).unwrap(), b"stub bytes");
        assert!(!has_part_file(&converter.tools_dir));
    }

    #[test]
    fn command_args_builds_the_to_zen_invocation() {
        let dir = tempfile::tempdir().unwrap();
        let pak = dir.path().join("MyMod_P.pak");
        let out_dir = dir.path().join("out");

        let args = command_args(&pak, &out_dir);

        assert_eq!(
            args,
            vec![
                std::ffi::OsString::from("to-zen"),
                std::ffi::OsString::from("--version"),
                std::ffi::OsString::from("UE5_1"),
                pak.clone().into_os_string(),
                out_dir.join("MyMod_P.utoc").into_os_string(),
            ]
        );
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn convert_is_unsupported_off_windows() {
        let (url, hits) = serve(Vec::new()).await;
        let app_root = tempfile::tempdir().unwrap();
        let converter = RetocConverter::new(app_root.path(), &url, &"0".repeat(64));
        let progress: Progress = &|_, _| {};
        let pak = app_root.path().join("Mod_P.pak");
        let out_dir = app_root.path().join("out");

        let error = converter
            .convert(&pak, &out_dir, progress)
            .await
            .unwrap_err();

        assert!(matches!(error, ConvertError::UnsupportedPlatform));
        assert!(!app_root.path().join("tools").exists());
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn convert_error_codes() {
        assert_eq!(
            ConvertError::UnsupportedPlatform.code(),
            "unsupported_platform"
        );
        assert_eq!(
            ConvertError::ToolUnavailable("x".to_string()).code(),
            "tool_unavailable"
        );
        assert_eq!(
            ConvertError::ConversionFailed("x".to_string()).code(),
            "conversion_failed"
        );
    }
}
