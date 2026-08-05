//! Native rfd-backed dialogs; psp-server's `desktop` feature only.

use std::path::PathBuf;

use crate::desktop_dialogs::{
    DialogFilesFuture, DialogFuture, FileDialogProvider, FileDialogRequest, FileSaveRequest,
};

/// Desktop mode: real native dialog via rfd. Gated behind the `desktop`
/// feature so the headless server/Docker build doesn't pull rfd's GUI deps.
pub struct RfdDialogProvider;

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
