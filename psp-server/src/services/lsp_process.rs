//! LSP stdio wire format and a supervisor for the `lua-language-server`
//! child process that backs the plugin editor's full tier.
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::emitter::Emitter;
use crate::messages::MessageType;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// No legitimate LSP header runs anywhere near this long; a buffer that grows
/// past it without ever finding a `\r\n\r\n` separator is not still-arriving
/// data, it is a desynchronised stream.
const MAX_HEADER_BYTES: usize = 64 * 1024;

/// Generous for the largest realistic single LSP message (a big
/// `textDocument/publishDiagnostics` or `workspace/symbol` reply), and small
/// enough that a hostile or corrupted declared length is rejected outright
/// instead of being trusted as a buffering target.
const MAX_CONTENT_LENGTH: usize = 64 * 1024 * 1024;

pub fn encode_frame(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("a serde_json::Value cannot fail to serialize");
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend(body);
    frame
}

/// The result of pulling one frame out of a `FrameReader`. Distinct from
/// `Option` because "a bad frame was dropped" and "not enough bytes yet" are
/// different situations for a consumer: more frames may already be sitting
/// in the buffer behind a dropped one, so a consumer must keep pulling on
/// `Dropped` the same as on `Frame`, and only stop on `Incomplete`.
#[derive(Debug)]
pub enum FrameOutcome {
    Frame(Value),
    Incomplete,
    Dropped,
    /// The buffered bytes can never resolve into a frame (a header that will
    /// never parse, or a declared length past `MAX_CONTENT_LENGTH`). The
    /// buffer has been discarded; the caller should stop feeding this reader.
    Fatal(String),
}

/// Buffers stdout bytes and yields complete LSP frames as they become
/// available, holding partial frames across calls to `push`.
pub struct FrameReader {
    buffer: Vec<u8>,
}

impl Default for FrameReader {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameReader {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Returns the next complete frame, or `None` while fewer than a full
    /// frame is buffered *or* the frame just pulled was dropped (a malformed
    /// body, or a header that can never parse). A production consumer that
    /// needs to distinguish those two cases — to keep draining frames queued
    /// up behind a dropped one — should use `next_outcome` instead.
    pub fn next_frame(&mut self) -> Option<Value> {
        match self.next_outcome() {
            FrameOutcome::Frame(value) => Some(value),
            FrameOutcome::Incomplete | FrameOutcome::Dropped | FrameOutcome::Fatal(_) => None,
        }
    }

    /// Returns the next complete frame, or `None` while fewer than a full
    /// frame is buffered. A frame whose body fails to parse as JSON is
    /// dropped (its exact bytes are drained) rather than left in the buffer
    /// or the buffer cleared, so the frames behind it are still delivered.
    pub fn next_outcome(&mut self) -> FrameOutcome {
        let separator = match find_separator(&self.buffer) {
            Some(index) => index,
            None => {
                return if self.buffer.len() > MAX_HEADER_BYTES {
                    self.buffer.clear();
                    FrameOutcome::Fatal(format!(
                        "no frame header separator found within {MAX_HEADER_BYTES} bytes"
                    ))
                } else {
                    FrameOutcome::Incomplete
                };
            }
        };
        // Bounds the header length as an invariant of the reader itself, not
        // as a side effect of how large the caller's `push` chunks happen to
        // be — a separator that does arrive, just very late, must be capped
        // the same as one that never arrives.
        if separator > MAX_HEADER_BYTES {
            self.buffer.clear();
            return FrameOutcome::Fatal(format!(
                "frame header exceeded the {MAX_HEADER_BYTES} byte limit"
            ));
        }
        let header_len = separator + 4;

        let header_text = match std::str::from_utf8(&self.buffer[..separator]) {
            Ok(text) => text,
            Err(_) => {
                self.buffer.clear();
                return FrameOutcome::Fatal("frame header was not valid UTF-8".to_string());
            }
        };

        let content_length = match parse_content_length(header_text) {
            Some(length) if length <= MAX_CONTENT_LENGTH => length,
            Some(_) => {
                self.buffer.clear();
                return FrameOutcome::Fatal(format!(
                    "declared Content-Length exceeds the {MAX_CONTENT_LENGTH} byte limit"
                ));
            }
            None => {
                self.buffer.clear();
                return FrameOutcome::Fatal(
                    "missing or unparsable Content-Length header".to_string(),
                );
            }
        };

        let total_len = match header_len.checked_add(content_length) {
            Some(total) => total,
            None => {
                self.buffer.clear();
                return FrameOutcome::Fatal("Content-Length overflowed".to_string());
            }
        };

        if self.buffer.len() < total_len {
            return FrameOutcome::Incomplete;
        }

        let frame_bytes: Vec<u8> = self.buffer.drain(..total_len).collect();
        match serde_json::from_slice(&frame_bytes[header_len..]) {
            Ok(value) => FrameOutcome::Frame(value),
            Err(_) => FrameOutcome::Dropped,
        }
    }
}

fn find_separator(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(header_text: &str) -> Option<usize> {
    header_text.split("\r\n").find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("Content-Length")
            .then(|| value.trim().parse().ok())
            .flatten()
    })
}

type PendingRequests = Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>;

/// The client a running child emits its unprompted frames to, resolved at
/// send time rather than captured at spawn. A page reload arrives as a new
/// connection while the child stays up, so a reader task holding the previous
/// connection's `Emitter` would emit into a dropped channel — and go on doing
/// so — for the rest of that child's life.
pub type ClientSlot = Arc<Mutex<Option<Emitter>>>;

pub fn emit_to_client<T: serde::Serialize>(
    client: &ClientSlot,
    message_type: MessageType,
    data: &T,
) {
    let emitter = client.lock().unwrap().clone();
    match emitter {
        Some(emitter) => emitter.emit(message_type, data),
        None => tracing::warn!(
            message_type = message_type.as_wire(),
            "no client is attached to the language server; its frame is dropped"
        ),
    }
}

/// Supervises a `lua-language-server` child process: a writer task drains
/// outgoing frames into its stdin, a reader task decodes frames from its
/// stdout and either completes a pending `request` or emits the frame to the
/// client as a notification.
pub struct LspProcess {
    outgoing: mpsc::UnboundedSender<Vec<u8>>,
    pending: PendingRequests,
    next_id: AtomicI64,
    /// Cleared by the reader task the moment it stops reading, for any
    /// reason (EOF, a read error, or a `Fatal` desync). `request` checks
    /// this before registering anything, so a dead process fails a caller
    /// immediately instead of every call paying the full request timeout.
    alive: Arc<AtomicBool>,
    child: Child,
    writer_task: JoinHandle<()>,
    reader_task: JoinHandle<()>,
}

/// The reader half of a supervised child: decodes frames off `stdout` and
/// either completes the `request` waiting on a frame's id or hands the frame
/// to the attached client.
///
/// Generic over the stream so a test can drive it over an in-memory pipe.
/// That is what pins the invariant below: the client is resolved from the
/// slot on every frame, so a connection that attaches after the reader
/// started still receives what the child says next.
async fn read_frames<R: tokio::io::AsyncRead + Unpin>(
    mut stdout: R,
    pending: PendingRequests,
    client: ClientSlot,
    plugin_id: String,
    alive: Arc<AtomicBool>,
) {
    let mut frame_reader = FrameReader::new();
    let mut chunk = [0u8; 8192];
    'read_loop: loop {
        let read = stdout.read(&mut chunk).await;
        let Ok(read_len) = read else { break };
        if read_len == 0 {
            break;
        }
        frame_reader.push(&chunk[..read_len]);
        loop {
            match frame_reader.next_outcome() {
                FrameOutcome::Frame(frame) => {
                    let id = frame.get("id").and_then(Value::as_i64);
                    let waiter = id.and_then(|id| pending.lock().unwrap().remove(&id));
                    match waiter {
                        Some(sender) => {
                            let _ = sender.send(frame);
                        }
                        None => emit_to_client(
                            &client,
                            MessageType::LspNotification,
                            &serde_json::json!({
                                "plugin_id": plugin_id,
                                "frame": frame,
                            }),
                        ),
                    }
                }
                // A dropped frame does not mean the stream ran dry —
                // frames queued up behind it must still be pulled.
                FrameOutcome::Dropped => continue,
                FrameOutcome::Incomplete => break,
                FrameOutcome::Fatal(reason) => {
                    tracing::error!(
                        %plugin_id,
                        %reason,
                        "lua-language-server stdout desynchronised; stopping reader"
                    );
                    break 'read_loop;
                }
            }
        }
    }
    // The child died, closed stdout, or desynchronised the stream: dropping
    // every pending sender fails the matching `request` awaits immediately
    // instead of hanging them, and clearing `alive` fails every future
    // `request` immediately too.
    pending.lock().unwrap().clear();
    alive.store(false, Ordering::SeqCst);
}

impl LspProcess {
    pub async fn spawn(
        exe: &Path,
        root: &Path,
        client: ClientSlot,
        plugin_id: String,
    ) -> Result<Self, String> {
        let mut child = Command::new(exe)
            .arg("--logpath")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("failed to spawn lua-language-server: {error}"))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "child stdin was not piped".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "child stdout was not piped".to_string())?;

        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let pending: PendingRequests = Arc::new(Mutex::new(HashMap::new()));
        let alive = Arc::new(AtomicBool::new(true));

        let writer_task = tokio::spawn(async move {
            while let Some(frame) = outgoing_rx.recv().await {
                if stdin.write_all(&frame).await.is_err() {
                    break;
                }
            }
        });

        let reader_task = tokio::spawn(read_frames(
            stdout,
            pending.clone(),
            client,
            plugin_id.clone(),
            alive.clone(),
        ));

        Ok(Self {
            outgoing: outgoing_tx,
            pending,
            next_id: AtomicI64::new(1),
            alive,
            child,
            writer_task,
            reader_task,
        })
    }

    /// False once the reader task has stopped, for any reason. A supervisor
    /// holding this process in a map needs it to evict a corpse: without it,
    /// the entry keeps answering "a process exists for this plugin" while
    /// every `request` against it fails.
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// `frame` must be a JSON object holding the LSP `method`/`params`
    /// (no `id`) — `request` assigns the id itself and overwrites any `id`
    /// the caller already set, so a caller matching on its own id would be
    /// silently mismatched.
    pub async fn request(&self, mut frame: Value) -> Result<Value, String> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err(
                "the language server process has already exited or desynchronised".to_string(),
            );
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        match frame.as_object_mut() {
            Some(object) => {
                object.insert("id".to_string(), Value::from(id));
            }
            None => return Err("an LSP request frame must be a JSON object".to_string()),
        }

        let (sender, receiver) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, sender);

        if self.outgoing.send(encode_frame(&frame)).is_err() {
            self.pending.lock().unwrap().remove(&id);
            return Err("the language server process is no longer running".to_string());
        }

        match tokio::time::timeout(REQUEST_TIMEOUT, receiver).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => Err("the language server process exited before responding".to_string()),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err("timed out waiting for a response from the language server".to_string())
            }
        }
    }

    pub async fn notify(&self, frame: Value) -> Result<(), String> {
        self.outgoing
            .send(encode_frame(&frame))
            .map_err(|_| "the language server process is no longer running".to_string())
    }

    pub async fn shutdown(mut self) {
        let shutdown_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let _ = self.outgoing.send(encode_frame(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": shutdown_id,
            "method": "shutdown",
            "params": Value::Null,
        })));
        let _ = self.outgoing.send(encode_frame(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "exit",
        })));

        if tokio::time::timeout(SHUTDOWN_TIMEOUT, self.child.wait())
            .await
            .is_err()
        {
            let _ = self.child.kill().await;
        }

        self.writer_task.abort();
        self.reader_task.abort();
    }
}

impl Drop for LspProcess {
    /// Covers a caller that drops `LspProcess` on an error path instead of
    /// awaiting `shutdown` — without this, `kill_on_drop` alone still kills
    /// the child, but the reader/writer tasks (and the reader's handle on the
    /// client slot) would otherwise leak for as long as the runtime is up.
    fn drop(&mut self) {
        self.writer_task.abort();
        self.reader_task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Any program that reads nothing and exits stands in for a language
    /// server whose child dies: `spawn` always passes `--logpath`, which every
    /// candidate here rejects or ignores before exiting.
    ///
    /// Panics rather than returning `None` when nothing matches. A stand-in
    /// this ordinary going missing is a broken environment, and a test that
    /// quietly skips itself reports success without having checked anything.
    fn a_program_that_exits_immediately() -> std::path::PathBuf {
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        if cfg!(windows) {
            let system_root = std::env::var_os("SYSTEMROOT")
                .unwrap_or_else(|| std::ffi::OsString::from(r"C:\Windows"));
            candidates.push(
                std::path::PathBuf::from(system_root)
                    .join("System32")
                    .join("whoami.exe"),
            );
        } else {
            for path in ["/bin/echo", "/usr/bin/echo", "/bin/true", "/usr/bin/true"] {
                candidates.push(std::path::PathBuf::from(path));
            }
        }
        match candidates.iter().find(|path| path.exists()) {
            Some(path) => path.clone(),
            None => panic!(
                "none of {candidates:?} exists, so the child-exit path cannot be exercised here"
            ),
        }
    }

    async fn next_frame_text(frames: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> String {
        tokio::time::timeout(Duration::from_secs(5), frames.recv())
            .await
            .expect("the reader task must emit within the timeout")
            .expect("the emitter is still open")
    }

    /// Drives the real reader task over an in-memory pipe, because the thing
    /// under test is *when* it resolves the client, and that is only
    /// observable across two frames with a reattach between them.
    #[tokio::test]
    async fn a_frame_goes_to_the_client_attached_when_it_arrives() {
        let (mut child_stdout_writer, child_stdout) = tokio::io::duplex(4096);
        let client: ClientSlot = Arc::new(Mutex::new(None));
        let pending: PendingRequests = Arc::new(Mutex::new(HashMap::new()));

        let (first, mut first_frames) = Emitter::test_channel();
        *client.lock().unwrap() = Some(first);

        let reader = tokio::spawn(read_frames(
            child_stdout,
            pending,
            Arc::clone(&client),
            "user.demo".to_string(),
            Arc::new(AtomicBool::new(true)),
        ));

        child_stdout_writer
            .write_all(&encode_frame(&serde_json::json!({
                "method": "textDocument/publishDiagnostics",
            })))
            .await
            .expect("write the first frame");
        let text = next_frame_text(&mut first_frames).await;
        assert!(
            text.contains("publishDiagnostics"),
            "the attached client must receive the first frame: {text}"
        );

        // The page reloaded: same child, still reading, but a new connection.
        let (second, mut second_frames) = Emitter::test_channel();
        *client.lock().unwrap() = Some(second);

        child_stdout_writer
            .write_all(&encode_frame(&serde_json::json!({
                "method": "window/logMessage",
            })))
            .await
            .expect("write the second frame");

        let text = next_frame_text(&mut second_frames).await;
        assert!(
            text.contains("logMessage"),
            "a reader that resolved its client once, when it started, would still be \
             emitting into the connection that went away: {text}"
        );
        assert!(
            first_frames.try_recv().is_err(),
            "the replaced connection must receive nothing after the reattach"
        );

        drop(child_stdout_writer);
        let _ = reader.await;
    }

    #[tokio::test]
    async fn is_alive_goes_false_once_the_child_is_gone() {
        let program = a_program_that_exits_immediately();
        let dir = tempfile::tempdir().expect("a temp dir");
        let (emitter, _frames) = Emitter::test_channel();
        let client: ClientSlot = Arc::new(Mutex::new(Some(emitter)));
        let process = LspProcess::spawn(&program, dir.path(), client, "user.demo".to_string())
            .await
            .expect("spawn");

        for _ in 0..200 {
            if !process.is_alive() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the reader task must clear `alive` when the child's stdout closes");
    }
}
