//! Native file dialog abstraction for desktop mode. The trait exists so handler
//! logic is testable without a display server (`QueuedDialogProvider`).

use std::collections::VecDeque;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Mutex;

pub struct FileDialogRequest {
    pub filter_name: &'static str,
    pub filter_extensions: &'static [&'static str],
    pub initial_directory: Option<PathBuf>,
}

/// A "save as" dialog: like `FileDialogRequest` but seeds the dialog with a
/// suggested file name the user can accept or override.
pub struct FileSaveRequest {
    pub filter_name: &'static str,
    pub filter_extensions: &'static [&'static str],
    pub suggested_file_name: String,
    pub initial_directory: Option<PathBuf>,
}

pub type DialogFuture = Pin<Box<dyn Future<Output = Option<PathBuf>> + Send>>;
pub type DialogFilesFuture = Pin<Box<dyn Future<Output = Option<Vec<PathBuf>>> + Send>>;

pub trait FileDialogProvider: Send + Sync {
    fn pick_file(&self, request: FileDialogRequest) -> DialogFuture;
    fn save_file(&self, request: FileSaveRequest) -> DialogFuture;
    fn pick_folder(&self, initial_directory: Option<PathBuf>) -> DialogFuture;
    fn pick_files(&self, request: FileDialogRequest) -> DialogFilesFuture;
}

/// Web mode: dialogs are never invoked; returns None if called anyway.
pub struct NullDialogProvider;

impl FileDialogProvider for NullDialogProvider {
    fn pick_file(&self, _request: FileDialogRequest) -> DialogFuture {
        Box::pin(async { None })
    }

    fn save_file(&self, _request: FileSaveRequest) -> DialogFuture {
        Box::pin(async { None })
    }

    fn pick_folder(&self, _initial_directory: Option<PathBuf>) -> DialogFuture {
        Box::pin(async { None })
    }

    fn pick_files(&self, _request: FileDialogRequest) -> DialogFilesFuture {
        Box::pin(async { None })
    }
}

/// Desktop mode: real native dialog via rfd. Gated behind the `desktop`
/// feature so the headless server/Docker build doesn't pull rfd's GUI deps.
#[cfg(feature = "desktop")]
pub struct RfdDialogProvider;

#[cfg(feature = "desktop")]
impl FileDialogProvider for RfdDialogProvider {
    fn pick_file(&self, request: FileDialogRequest) -> DialogFuture {
        Box::pin(async move {
            let mut dialog = rfd::AsyncFileDialog::new()
                .add_filter(request.filter_name, request.filter_extensions)
                .add_filter("All files", &["*"]);
            if let Some(directory) = &request.initial_directory {
                if directory.is_dir() {
                    dialog = dialog.set_directory(directory);
                }
            }
            dialog
                .pick_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        })
    }

    fn save_file(&self, request: FileSaveRequest) -> DialogFuture {
        Box::pin(async move {
            let mut dialog = rfd::AsyncFileDialog::new()
                .add_filter(request.filter_name, request.filter_extensions)
                .add_filter("All files", &["*"])
                .set_file_name(request.suggested_file_name);
            if let Some(directory) = &request.initial_directory {
                if directory.is_dir() {
                    dialog = dialog.set_directory(directory);
                }
            }
            dialog
                .save_file()
                .await
                .map(|handle| handle.path().to_path_buf())
        })
    }

    fn pick_folder(&self, initial_directory: Option<PathBuf>) -> DialogFuture {
        Box::pin(async move {
            let mut dialog = rfd::AsyncFileDialog::new();
            if let Some(directory) = &initial_directory {
                if directory.is_dir() {
                    dialog = dialog.set_directory(directory);
                }
            }
            dialog
                .pick_folder()
                .await
                .map(|handle| handle.path().to_path_buf())
        })
    }

    fn pick_files(&self, request: FileDialogRequest) -> DialogFilesFuture {
        Box::pin(async move {
            let mut dialog = rfd::AsyncFileDialog::new()
                .add_filter(request.filter_name, request.filter_extensions)
                .add_filter("All files", &["*"]);
            if let Some(directory) = &request.initial_directory {
                if directory.is_dir() {
                    dialog = dialog.set_directory(directory);
                }
            }
            dialog.pick_files().await.map(|handles| {
                handles
                    .iter()
                    .map(|handle| handle.path().to_path_buf())
                    .collect()
            })
        })
    }
}

/// Test double: pops pre-queued answers in order; None simulates a canceled dialog.
/// `pick_file` and `save_file` draw from separate queues.
pub struct QueuedDialogProvider {
    queued_pick_responses: Mutex<VecDeque<Option<PathBuf>>>,
    queued_pick_files_responses: Mutex<VecDeque<Option<Vec<PathBuf>>>>,
    queued_save_responses: Mutex<VecDeque<Option<PathBuf>>>,
    queued_folder_responses: Mutex<VecDeque<Option<PathBuf>>>,
}

impl QueuedDialogProvider {
    pub fn new(pick_responses: Vec<Option<PathBuf>>) -> Self {
        Self::new_with_all(pick_responses, Vec::new(), Vec::new())
    }

    pub fn new_with_saves(
        pick_responses: Vec<Option<PathBuf>>,
        save_responses: Vec<Option<PathBuf>>,
    ) -> Self {
        Self::new_with_all(pick_responses, save_responses, Vec::new())
    }

    /// Folder-pick answers only; `pick_file`/`save_file` queues stay empty.
    pub fn new_with_folders(folder_responses: Vec<Option<PathBuf>>) -> Self {
        Self::new_with_all(Vec::new(), Vec::new(), folder_responses)
    }

    /// Seeds all three queues; each dialog kind draws from its own.
    pub fn new_with_all(
        pick_responses: Vec<Option<PathBuf>>,
        save_responses: Vec<Option<PathBuf>>,
        folder_responses: Vec<Option<PathBuf>>,
    ) -> Self {
        Self {
            queued_pick_responses: Mutex::new(pick_responses.into()),
            queued_pick_files_responses: Mutex::new(VecDeque::new()),
            queued_save_responses: Mutex::new(save_responses.into()),
            queued_folder_responses: Mutex::new(folder_responses.into()),
        }
    }

    /// Multi-file pick answers only; the other three queues stay empty.
    pub fn new_with_pick_files(pick_files_responses: Vec<Option<Vec<PathBuf>>>) -> Self {
        Self {
            queued_pick_responses: Mutex::new(VecDeque::new()),
            queued_pick_files_responses: Mutex::new(pick_files_responses.into()),
            queued_save_responses: Mutex::new(VecDeque::new()),
            queued_folder_responses: Mutex::new(VecDeque::new()),
        }
    }

    /// Seeds both the save queue (for export) and the multi-file pick queue
    /// (for import), so a single provider can drive an export-then-import
    /// round trip in one test. `pick_file`/`pick_folder` queues stay empty.
    pub fn new_with_saves_and_pick_files(
        save_responses: Vec<Option<PathBuf>>,
        pick_files_responses: Vec<Option<Vec<PathBuf>>>,
    ) -> Self {
        Self {
            queued_pick_responses: Mutex::new(VecDeque::new()),
            queued_pick_files_responses: Mutex::new(pick_files_responses.into()),
            queued_save_responses: Mutex::new(save_responses.into()),
            queued_folder_responses: Mutex::new(VecDeque::new()),
        }
    }
}

impl FileDialogProvider for QueuedDialogProvider {
    fn pick_file(&self, _request: FileDialogRequest) -> DialogFuture {
        let response = self
            .queued_pick_responses
            .lock()
            .expect("queued dialog mutex poisoned")
            .pop_front()
            .flatten();
        Box::pin(async move { response })
    }

    fn save_file(&self, _request: FileSaveRequest) -> DialogFuture {
        let response = self
            .queued_save_responses
            .lock()
            .expect("queued dialog mutex poisoned")
            .pop_front()
            .flatten();
        Box::pin(async move { response })
    }

    fn pick_folder(&self, _initial_directory: Option<PathBuf>) -> DialogFuture {
        let response = self
            .queued_folder_responses
            .lock()
            .expect("queued dialog mutex poisoned")
            .pop_front()
            .flatten();
        Box::pin(async move { response })
    }

    fn pick_files(&self, _request: FileDialogRequest) -> DialogFilesFuture {
        let response = self
            .queued_pick_files_responses
            .lock()
            .expect("queued dialog mutex poisoned")
            .pop_front()
            .flatten();
        Box::pin(async move { response })
    }
}

/// Platform default directory for Steam saves.
pub fn steam_save_root() -> PathBuf {
    if cfg!(target_os = "windows") {
        let local_app_data = std::env::var_os("LOCALAPPDATA").unwrap_or_default();
        Path::new(&local_app_data)
            .join("Pal")
            .join("Saved")
            .join("SaveGames")
    } else if cfg!(target_os = "macos") {
        let user = std::env::var("USER").unwrap_or_default();
        PathBuf::from(format!(
            "/System/Volumes/Data/Users/{user}/Library/Containers/com.pocketpair.palworld.mac/Data/Library/Application Support/Epic/Pal/Saved/SaveGames"
        ))
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
    }
}

/// Platform default directory for Game Pass save containers.
pub fn gamepass_save_root() -> PathBuf {
    if cfg!(target_os = "windows") {
        let local_app_data = std::env::var_os("LOCALAPPDATA").unwrap_or_default();
        Path::new(&local_app_data)
            .join("Packages")
            .join("PocketpairInc.Palworld_ad4psfrxyesvt")
            .join("SystemAppData")
            .join("wgs")
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
    }
}

/// Filter + initial dir per save type; a save_dir stored in settings overrides
/// the platform default.
pub fn dialog_request_for(save_type: &str, saved_save_dir: Option<&str>) -> FileDialogRequest {
    let (filter_name, filter_extensions, default_root): (
        &'static str,
        &'static [&'static str],
        PathBuf,
    ) = match save_type {
        "steam" | "local_data" => ("Sav Files", &["sav"], steam_save_root()),
        _ => ("Container Index Files", &["index"], gamepass_save_root()),
    };
    FileDialogRequest {
        filter_name,
        filter_extensions,
        initial_directory: Some(saved_save_dir.map(PathBuf::from).unwrap_or(default_root)),
    }
}

/// App-dir guard + expected-filename check. The error strings are wire-visible:
/// they are sent verbatim as the `error` message the frontend renders.
pub fn validate_selected_file(
    save_type: &str,
    selected: &Path,
    app_root: &Path,
) -> Result<(), String> {
    if selected.starts_with(app_root) {
        return Err(
            "Selected path is inside the PSP application directory. Please move your save files outside of the application directory."
                .to_string(),
        );
    }
    let file_name = selected
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let expected = match save_type {
        "steam" => "Level.sav",
        "local_data" => "LocalData.sav",
        _ => "containers.index",
    };
    if file_name == expected {
        Ok(())
    } else {
        Err(format!(
            "Selected file {file_name} does not match expected type for {save_type} save. Please select a valid save file."
        ))
    }
}

/// The directory containing the running executable. Used only for the "picked a
/// file inside the app dir" guard.
pub fn application_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe_path| exe_path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn steam_selection_accepts_level_sav() {
        let selected = Path::new("/saves/ABC123/Level.sav");
        let app_root = Path::new("/opt/psp");
        assert_eq!(validate_selected_file("steam", selected, app_root), Ok(()));
    }

    #[test]
    fn steam_selection_rejects_other_filenames_with_exact_python_message() {
        let selected = Path::new("/saves/ABC123/LevelMeta.sav");
        let app_root = Path::new("/opt/psp");
        assert_eq!(
            validate_selected_file("steam", selected, app_root),
            Err("Selected file LevelMeta.sav does not match expected type for steam save. Please select a valid save file.".to_string())
        );
    }

    #[test]
    fn gamepass_selection_accepts_containers_index() {
        let selected = Path::new("/wgs/containers.index");
        let app_root = Path::new("/opt/psp");
        assert_eq!(
            validate_selected_file("gamepass", selected, app_root),
            Ok(())
        );
    }

    #[test]
    fn local_data_selection_accepts_local_data_sav() {
        let selected = Path::new("/saves/LocalData.sav");
        let app_root = Path::new("/opt/psp");
        assert_eq!(
            validate_selected_file("local_data", selected, app_root),
            Ok(())
        );
    }

    #[test]
    fn selection_inside_app_root_is_rejected_with_exact_python_message() {
        let selected = Path::new("/opt/psp/backups/Level.sav");
        let app_root = Path::new("/opt/psp");
        assert_eq!(
            validate_selected_file("steam", selected, app_root),
            Err("Selected path is inside the PSP application directory. Please move your save files outside of the application directory.".to_string())
        );
    }

    #[test]
    fn dialog_request_uses_sav_filter_for_steam_and_index_filter_for_gamepass() {
        let steam_request = dialog_request_for("steam", None);
        assert_eq!(steam_request.filter_name, "Sav Files");
        assert_eq!(steam_request.filter_extensions, &["sav"]);
        assert_eq!(steam_request.initial_directory, Some(steam_save_root()));

        let gamepass_request = dialog_request_for("gamepass", None);
        assert_eq!(gamepass_request.filter_name, "Container Index Files");
        assert_eq!(gamepass_request.filter_extensions, &["index"]);
        assert_eq!(
            gamepass_request.initial_directory,
            Some(gamepass_save_root())
        );
    }

    #[test]
    fn dialog_request_prefers_saved_save_dir_over_default_root() {
        let request = dialog_request_for("steam", Some("/my/saves"));
        assert_eq!(request.initial_directory, Some(PathBuf::from("/my/saves")));
    }

    #[tokio::test]
    async fn queued_provider_pops_responses_in_order() {
        let provider = QueuedDialogProvider::new(vec![Some(PathBuf::from("/a/Level.sav")), None]);
        assert_eq!(
            provider.pick_file(dialog_request_for("steam", None)).await,
            Some(PathBuf::from("/a/Level.sav"))
        );
        assert_eq!(
            provider.pick_file(dialog_request_for("steam", None)).await,
            None
        );
    }

    #[tokio::test]
    async fn queued_provider_draws_pick_and_save_from_separate_queues() {
        let provider = QueuedDialogProvider::new_with_saves(
            vec![Some(PathBuf::from("/pick/in.json"))],
            vec![Some(PathBuf::from("/save/out.json"))],
        );
        let save_request = FileSaveRequest {
            filter_name: "Preset Files",
            filter_extensions: &["json"],
            suggested_file_name: "out.json".to_string(),
            initial_directory: None,
        };
        assert_eq!(
            provider.save_file(save_request).await,
            Some(PathBuf::from("/save/out.json"))
        );
        assert_eq!(
            provider.pick_file(dialog_request_for("steam", None)).await,
            Some(PathBuf::from("/pick/in.json"))
        );
    }

    #[tokio::test]
    async fn queued_provider_pops_pick_files_in_order() {
        let provider = QueuedDialogProvider::new_with_pick_files(vec![
            Some(vec![PathBuf::from("/a.json"), PathBuf::from("/b.zip")]),
            None,
        ]);
        let request = FileDialogRequest {
            filter_name: "Preset Files",
            filter_extensions: &["zip", "json"],
            initial_directory: None,
        };
        assert_eq!(
            provider.pick_files(request).await,
            Some(vec![PathBuf::from("/a.json"), PathBuf::from("/b.zip")])
        );
        let request2 = FileDialogRequest {
            filter_name: "Preset Files",
            filter_extensions: &["zip", "json"],
            initial_directory: None,
        };
        assert_eq!(provider.pick_files(request2).await, None);
    }
}
