pub fn fake_windows_install() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let binaries = dir.path().join("Pal/Binaries/Win64");
    std::fs::create_dir_all(&binaries).unwrap();
    std::fs::write(binaries.join("Palworld-Win64-Shipping.exe"), b"stub").unwrap();
    // Marks this install as running standard-mode UE4SS, so `detect_ue4ss`
    // resolves `ue4ss_mods_dir` instead of leaving it `None`; without this a
    // scan never looks under `ue4ss/Mods` at all.
    std::fs::write(binaries.join("dwmapi.dll"), b"stub").unwrap();
    std::fs::create_dir_all(dir.path().join("Pal/Content/Paks")).unwrap();
    dir
}

pub fn ue4ss_mods_dir(root: &std::path::Path) -> std::path::PathBuf {
    root.join("Pal/Binaries/Win64/ue4ss/Mods")
}

pub fn write_ue4ss_mod(root: &std::path::Path, name: &str, file: &str, bytes: &[u8]) {
    let dir = ue4ss_mods_dir(root).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(file), bytes).unwrap();
}

/// Reads frames until one of `wanted` type arrives, returning it together with
/// every frame skipped on the way (pushes such as `mod_progress` interleave with
/// replies).
pub async fn next_of_type(
    ws: &mut super::WsClient,
    wanted: &str,
) -> (serde_json::Value, Vec<serde_json::Value>) {
    let mut skipped = Vec::new();
    loop {
        let frame = super::next_json(ws).await;
        if frame["type"] == wanted {
            return (frame, skipped);
        }
        skipped.push(frame);
    }
}
