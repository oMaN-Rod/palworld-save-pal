use ps_core::mods::{DesiredFile, InstallManifest, Role, RouteKind, TargetLayout};
use ps_db::mod_targets::ModTarget;

use super::super::digest;
use super::super::layout::{self, LayoutResolveError};
use super::super::library;
use super::super::paths::LibraryPaths;

#[derive(Debug, thiserror::Error)]
pub enum DesiredError {
    #[error("mod {0} has no current version and the profile pins none")]
    NoVersion(String),
    #[error("mod version {0} is not in the library")]
    MissingVersion(String),
    #[error("mod version {0} has an unreadable manifest: {1}")]
    BadManifest(String, String),
    #[error("route kind {0:?} has no home on this target")]
    NoBaseForKind(RouteKind),
    #[error(transparent)]
    Layout(#[from] LayoutResolveError),
    #[error(transparent)]
    Db(#[from] ps_db::DbError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Default)]
pub struct DesiredSet {
    pub files: Vec<DesiredFile>,
    /// UE4SS entries in profile order, for `mods.txt`.
    pub ue4ss_entries: Vec<(String, bool)>,
    /// Workshop package names that are enabled, for `PalModSettings.ini`.
    pub workshop_active: Vec<String>,
    /// The package name of every Workshop version this target holds in a profile,
    /// has deployed, or has released: the only `ActiveModList` lines an apply may
    /// remove.
    pub workshop_managed: Vec<String>,
    /// The released packages folded into `workshop_managed`, forgotten once an
    /// apply has written them out.
    pub workshop_released: Vec<String>,
    /// Folder names of managed UE4SS mods that are *not* enabled, so their
    /// `mods.txt` lines are purged rather than left behind.
    pub ue4ss_purge: Vec<String>,
    /// A UE4SS framework slot exists, so a missing `mods.txt` starts from UE4SS's own.
    pub seed_ue4ss_mods_txt: bool,
}

struct Selected {
    version: ps_db::mod_library::ModVersionRow,
    manifest: InstallManifest,
    enabled: bool,
}

async fn resolve_version(
    db: &dyn ps_db::DbDriver,
    mod_id: &str,
    pinned: Option<&str>,
) -> Result<ps_db::mod_library::ModVersionRow, DesiredError> {
    match pinned {
        Some(id) => ps_db::mod_library::get_version(db, id)
            .await?
            .ok_or_else(|| DesiredError::MissingVersion(id.to_string())),
        None => ps_db::mod_library::current_version(db, mod_id)
            .await?
            .ok_or_else(|| DesiredError::NoVersion(mod_id.to_string())),
    }
}

fn manifest_of(
    version: &ps_db::mod_library::ModVersionRow,
) -> Result<InstallManifest, DesiredError> {
    serde_json::from_str(&version.manifest)
        .map_err(|e| DesiredError::BadManifest(version.id.clone(), e.to_string()))
}

/// Resolves a route to its absolute destination through the layout, which is the
/// only place a target path may come from.
fn destination(
    layout: &TargetLayout,
    kind: RouteKind,
    rel_path: &str,
) -> Result<std::path::PathBuf, DesiredError> {
    let base = layout
        .base_for(kind)
        .ok_or(DesiredError::NoBaseForKind(kind))?;
    let mut path = base.to_path_buf();
    for segment in rel_path.split('/').filter(|s| !s.is_empty()) {
        path.push(segment);
    }
    Ok(path)
}

/// Resolved against the directory the row recorded, never against one recomputed
/// from the version string: a collision may have forced a different name.
fn library_source(
    version: &ps_db::mod_library::ModVersionRow,
    kind: RouteKind,
    rel_path: &str,
) -> std::path::PathBuf {
    LibraryPaths::route_path_in(std::path::Path::new(&version.library_dir), kind, rel_path)
}

/// No `LibraryPaths` argument: every source path comes from the version row's own
/// `library_dir`, which is where the bytes actually are even if the app root moved.
pub async fn build(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    profile_id: &str,
) -> Result<DesiredSet, DesiredError> {
    let layout = layout::layout_for(target)?;
    let profile = ps_db::mod_profiles::get(db, profile_id)
        .await?
        .ok_or_else(|| ps_db::DbError::Other(format!("profile {profile_id} not found")))?;
    let drop_ue4ss_markers = profile.force_order_ue4ss || profile.ue4ss_control_mode == "mods_txt";
    let mut palschema_rank = 0usize;
    let mut selected: Vec<Selected> = Vec::new();

    for entry in ps_db::mod_profiles::mods_of(db, profile_id).await? {
        let version = resolve_version(db, &entry.mod_id, entry.mod_version_id.as_deref()).await?;
        let manifest = manifest_of(&version)?;
        selected.push(Selected {
            version,
            manifest,
            enabled: entry.enabled,
        });
    }

    // Frameworks are target state, not profile state: switching profiles must
    // never plan a framework removal.
    let mut seed_ue4ss_mods_txt = false;
    for slot in ps_db::mod_profiles::frameworks_of(db, &target.id).await? {
        if slot.framework == "ue4ss" {
            seed_ue4ss_mods_txt = true;
        }
        let version = ps_db::mod_library::get_version(db, &slot.mod_version_id)
            .await?
            .ok_or_else(|| DesiredError::MissingVersion(slot.mod_version_id.clone()))?;
        let manifest = manifest_of(&version)?;
        selected.push(Selected {
            version,
            manifest,
            enabled: true,
        });
    }

    let mut set = DesiredSet {
        seed_ue4ss_mods_txt,
        ..DesiredSet::default()
    };
    for item in &selected {
        let is_framework = item.manifest.mod_type == ps_core::mods::ModType::Framework;
        if !is_framework {
            match item.manifest.mod_type {
                ps_core::mods::ModType::Workshop => {
                    if item.enabled {
                        // A package with no files of its own is activated through
                        // the settings file alone, so a layout with no Workshop
                        // home would accept it and never load it.
                        if layout.base_for(RouteKind::Workshop).is_none() {
                            return Err(DesiredError::NoBaseForKind(RouteKind::Workshop));
                        }
                        set.workshop_active.push(item.manifest.folder_name.clone());
                    }
                }
                ps_core::mods::ModType::Ue4ss | ps_core::mods::ModType::Hybrid => {
                    set.ue4ss_entries
                        .push((item.manifest.folder_name.clone(), item.enabled));
                }
                _ => {}
            }
        }
        if !item.enabled {
            // A disabled mod keeps its marker line and contributes no files; the
            // diff turns its recorded files into `Remove`.
            continue;
        }
        let prefix = (profile.force_order_palschema
            && !is_framework
            && item.manifest.mod_type == ps_core::mods::ModType::PalSchema)
            .then(|| {
                palschema_rank += 1;
                format!("{palschema_rank:03}_")
            });
        for route in &item.manifest.routes {
            let role = role_for(route.kind, &route.rel_path);
            if drop_ue4ss_markers
                && !is_framework
                && route.kind == RouteKind::Ue4ss
                && role == Role::Marker
            {
                continue;
            }
            let deployed_rel = match (&prefix, route.kind) {
                (Some(prefix), RouteKind::PalSchema) => format!("{prefix}{}", route.rel_path),
                _ => route.rel_path.clone(),
            };
            let path = destination(&layout, route.kind, &deployed_rel)?;
            let source = library_source(&item.version, route.kind, &route.rel_path);
            set.files.push(DesiredFile {
                path: path.to_string_lossy().into_owned(),
                mod_version_id: item.version.id.clone(),
                rel_path: route.rel_path.clone(),
                kind: route.kind,
                // Read now rather than stored, so it stays correct even if
                // something rewrote the library copy.
                expected_hash: digest::hash_file(&source)?,
                role,
                source: source.to_string_lossy().into_owned(),
            });
        }
    }

    // A version whose manifest cannot be read names no package, so no line is
    // treated as belonging to it. A package this target neither holds nor has
    // deployed is another target's or Steam's line here.
    let on_target = library::mods_on_target(db, &target.id).await?;
    for library_mod in ps_db::mod_library::list_mods(db).await? {
        if !on_target.contains(&library_mod.id) {
            continue;
        }
        for version in ps_db::mod_library::versions_of(db, &library_mod.id).await? {
            let Ok(manifest) = manifest_of(&version) else {
                continue;
            };
            if manifest.mod_type == ps_core::mods::ModType::Workshop
                && !set.workshop_managed.contains(&manifest.folder_name)
            {
                set.workshop_managed.push(manifest.folder_name);
            }
        }
    }
    set.workshop_released = library::released_packages(db, &target.id).await?;
    for name in &set.workshop_released {
        if !set.workshop_managed.contains(name) {
            set.workshop_managed.push(name.clone());
        }
    }

    set.ue4ss_purge = set
        .ue4ss_entries
        .iter()
        .filter(|(_, enabled)| !enabled)
        .map(|(name, _)| name.clone())
        .collect();
    Ok(set)
}

/// Finds a desired file by normalised key rather than by string: a path that
/// reaches this from a journal or a row can differ from the one `build` produced
/// by separators — a docker target's forward-slashed `root_path` — or by case on
/// a case-insensitive filesystem.
pub fn file_at<'a>(files: &'a [DesiredFile], path: &std::path::Path) -> Option<&'a DesiredFile> {
    let key = super::apply::path_key(&path.to_string_lossy());
    files
        .iter()
        .find(|f| super::apply::path_key(&f.path) == key)
}

/// A `.ucas` or `.utoc` beside a pak is a companion; an `enabled.txt` inside a mod
/// folder is that mod version's own marker. Everything else is an ordinary file.
fn role_for(kind: RouteKind, rel_path: &str) -> Role {
    let leaf = rel_path.rsplit('/').next().unwrap_or("");
    if leaf.eq_ignore_ascii_case("enabled.txt") {
        return Role::Marker;
    }
    if matches!(kind, RouteKind::Pak | RouteKind::LogicMods)
        && (leaf.ends_with(".ucas") || leaf.ends_with(".utoc"))
    {
        return Role::Companion;
    }
    Role::File
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use ps_core::mods::{FileRoute, ModType, SourceHint};
    use ps_db::mod_targets::NewModTarget;

    use super::*;

    struct Fixture {
        db: ps_db::SqlxSqliteDriver,
        paths: LibraryPaths,
        target: ModTarget,
        profile_id: String,
        _dir: tempfile::TempDir,
    }

    fn write(path: &Path, body: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    async fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let db =
            ps_db::SqlxSqliteDriver::new(ps_db::open(&dir.path().join("test.db")).await.unwrap());
        let root = dir.path().join("game");
        write(
            &root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"),
            b"x",
        );
        std::fs::create_dir_all(root.join("Pal/Binaries/Win64/ue4ss/Mods")).unwrap();
        std::fs::create_dir_all(root.join("Pal/Content/Paks/~mods")).unwrap();
        let target = ps_db::mod_targets::upsert(
            &db,
            &NewModTarget {
                id: "client-steam".to_string(),
                kind: "client".to_string(),
                name: "Steam".to_string(),
                root_path: root.to_string_lossy().into_owned(),
                platform: "win64".to_string(),
                ue4ss_mode: "standard".to_string(),
                layout_overrides: "{}".to_string(),
                detected: "{}".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let profile_id = format!("{}/default", target.id);
        ps_db::mod_profiles::create(
            &db,
            &ps_db::mod_profiles::NewProfile {
                id: profile_id.clone(),
                target_id: target.id.clone(),
                name: "Default".to_string(),
                is_default: true,
            },
        )
        .await
        .unwrap();
        Fixture {
            paths: LibraryPaths::new(&dir.path().join("app")),
            db,
            target,
            profile_id,
            _dir: dir,
        }
    }

    impl Fixture {
        async fn add_mod(
            &self,
            mod_id: &str,
            mod_type: ModType,
            routes: &[(RouteKind, &str)],
            enabled: bool,
            load_order: i64,
        ) {
            let scratch = tempfile::tempdir().unwrap();
            for (_, rel_path) in routes {
                write(&scratch.path().join(rel_path), rel_path.as_bytes());
            }
            let folder = routes[0].1.split('/').next().unwrap().to_string();
            let manifest = InstallManifest {
                folder_name: folder.clone(),
                display_name: folder,
                mod_type,
                version: "1.0".to_string(),
                routes: routes
                    .iter()
                    .map(|(kind, rel_path)| FileRoute {
                        archive_path: rel_path.to_string(),
                        rel_path: rel_path.to_string(),
                        kind: *kind,
                    })
                    .collect(),
                decisions: Vec::new(),
                platform_filtered: None,
                source: SourceHint::default(),
            };
            library::store(
                &self.db,
                &self.paths,
                &library::StoreRequest {
                    mod_id,
                    manifest: &manifest,
                    extracted_root: scratch.path(),
                    archive: None,
                    source_kind: "local",
                    source_ref: "{}",
                    custom_name: None,
                },
            )
            .await
            .unwrap();
            ps_db::mod_profiles::set_mod(
                &self.db,
                &ps_db::mod_profiles::ProfileModRow {
                    profile_id: self.profile_id.clone(),
                    mod_id: mod_id.to_string(),
                    mod_version_id: None,
                    enabled,
                    load_order,
                },
            )
            .await
            .unwrap();
        }

        async fn add_framework(&self, key: &str, routes: &[(RouteKind, &str)]) -> String {
            let scratch = tempfile::tempdir().unwrap();
            for (_, rel_path) in routes {
                write(&scratch.path().join(rel_path), rel_path.as_bytes());
            }
            let manifest = InstallManifest {
                folder_name: key.to_string(),
                display_name: key.to_string(),
                mod_type: ModType::Framework,
                version: "1.0".to_string(),
                routes: routes
                    .iter()
                    .map(|(kind, rel_path)| FileRoute {
                        archive_path: rel_path.to_string(),
                        rel_path: rel_path.to_string(),
                        kind: *kind,
                    })
                    .collect(),
                decisions: Vec::new(),
                platform_filtered: None,
                source: SourceHint::default(),
            };
            let mod_id = format!("framework-{key}");
            let stored = library::store(
                &self.db,
                &self.paths,
                &library::StoreRequest {
                    mod_id: &mod_id,
                    manifest: &manifest,
                    extracted_root: scratch.path(),
                    archive: None,
                    source_kind: "framework",
                    source_ref: "{}",
                    custom_name: None,
                },
            )
            .await
            .unwrap();
            ps_db::mod_profiles::set_framework(&self.db, &self.target.id, key, &stored.version.id)
                .await
                .unwrap();
            stored.version.id
        }

        async fn set_options(&self, control_mode: &str, force_ue4ss: bool, force_palschema: bool) {
            ps_db::mod_profiles::set_options(
                &self.db,
                &self.profile_id,
                control_mode,
                force_ue4ss,
                force_palschema,
            )
            .await
            .unwrap();
        }

        async fn build(&self) -> DesiredSet {
            build(&self.db, &self.target, &self.profile_id)
                .await
                .unwrap()
        }

        fn palschema_path(&self, rel_path: &str) -> PathBuf {
            let layout = layout::layout_for(&self.target).unwrap();
            let mut path = layout.palschema_mods_dir.unwrap();
            for segment in rel_path.split('/') {
                path.push(segment);
            }
            path
        }
    }

    fn file_by_rel<'a>(set: &'a DesiredSet, rel_path: &str) -> Option<&'a DesiredFile> {
        set.files.iter().find(|f| f.rel_path == rel_path)
    }

    fn rel_paths(set: &DesiredSet) -> Vec<&str> {
        set.files.iter().map(|f| f.rel_path.as_str()).collect()
    }

    async fn b_then_a(fixture: &Fixture, b_enabled: bool) {
        fixture
            .add_mod(
                "b",
                ModType::PalSchema,
                &[(RouteKind::PalSchema, "B/y.json")],
                b_enabled,
                0,
            )
            .await;
        fixture
            .add_mod(
                "a",
                ModType::PalSchema,
                &[(RouteKind::PalSchema, "A/x.json")],
                true,
                1,
            )
            .await;
    }

    async fn cool_mod(fixture: &Fixture) {
        fixture
            .add_mod(
                "coolmod",
                ModType::Ue4ss,
                &[
                    (RouteKind::Ue4ss, "CoolMod/enabled.txt"),
                    (RouteKind::Ue4ss, "CoolMod/Scripts/main.lua"),
                ],
                true,
                0,
            )
            .await;
    }

    #[tokio::test]
    async fn palschema_force_order_prefixes_enabled_palschema_mods_in_load_order() {
        let fixture = fixture().await;
        b_then_a(&fixture, true).await;
        fixture.set_options("enabled_txt", false, true).await;

        let set = fixture.build().await;

        let b = file_by_rel(&set, "B/y.json").unwrap();
        let a = file_by_rel(&set, "A/x.json").unwrap();
        assert_eq!(Path::new(&b.path), fixture.palschema_path("001_B/y.json"));
        assert_eq!(Path::new(&a.path), fixture.palschema_path("002_A/x.json"));
    }

    #[tokio::test]
    async fn a_disabled_palschema_mod_takes_no_prefix_number() {
        let fixture = fixture().await;
        b_then_a(&fixture, false).await;
        fixture.set_options("enabled_txt", false, true).await;

        let set = fixture.build().await;

        assert!(file_by_rel(&set, "B/y.json").is_none());
        let a = file_by_rel(&set, "A/x.json").unwrap();
        assert_eq!(Path::new(&a.path), fixture.palschema_path("001_A/x.json"));
    }

    #[tokio::test]
    async fn without_force_order_palschema_paths_are_unprefixed() {
        let fixture = fixture().await;
        b_then_a(&fixture, true).await;

        let set = fixture.build().await;

        let b = file_by_rel(&set, "B/y.json").unwrap();
        let a = file_by_rel(&set, "A/x.json").unwrap();
        assert_eq!(Path::new(&b.path), fixture.palschema_path("B/y.json"));
        assert_eq!(Path::new(&a.path), fixture.palschema_path("A/x.json"));
    }

    #[tokio::test]
    async fn force_order_ue4ss_drops_enabled_txt_markers() {
        let fixture = fixture().await;
        cool_mod(&fixture).await;
        fixture.set_options("enabled_txt", true, false).await;

        let set = fixture.build().await;

        assert_eq!(rel_paths(&set), ["CoolMod/Scripts/main.lua"]);
        assert_eq!(set.ue4ss_entries, [("CoolMod".to_string(), true)]);
    }

    #[tokio::test]
    async fn mods_txt_control_mode_drops_enabled_txt_markers() {
        let fixture = fixture().await;
        cool_mod(&fixture).await;
        fixture.set_options("mods_txt", false, false).await;

        let set = fixture.build().await;

        assert_eq!(rel_paths(&set), ["CoolMod/Scripts/main.lua"]);
        assert_eq!(set.ue4ss_entries, [("CoolMod".to_string(), true)]);
    }

    #[tokio::test]
    async fn force_order_keeps_framework_enabled_txt_markers() {
        let fixture = fixture().await;
        cool_mod(&fixture).await;
        fixture
            .add_framework(
                "palschema",
                &[
                    (RouteKind::Ue4ss, "PalSchema/enabled.txt"),
                    (RouteKind::Ue4ss, "PalSchema/dlls/main.dll"),
                ],
            )
            .await;
        fixture.set_options("mods_txt", true, false).await;

        let set = fixture.build().await;

        assert!(
            file_by_rel(&set, "PalSchema/enabled.txt").is_some(),
            "{:?}",
            rel_paths(&set)
        );
        assert!(file_by_rel(&set, "CoolMod/enabled.txt").is_none());
    }

    #[tokio::test]
    async fn a_ue4ss_slot_asks_for_the_default_mods_txt() {
        let fixture = fixture().await;
        cool_mod(&fixture).await;

        let without = fixture.build().await;
        assert!(!without.seed_ue4ss_mods_txt);

        fixture
            .add_framework("ue4ss", &[(RouteKind::Binaries, "dwmapi.dll")])
            .await;
        let with = fixture.build().await;
        assert!(with.seed_ue4ss_mods_txt);
    }

    #[tokio::test]
    async fn enabled_txt_mode_keeps_markers() {
        let fixture = fixture().await;
        cool_mod(&fixture).await;

        let set = fixture.build().await;

        assert_eq!(
            rel_paths(&set),
            ["CoolMod/enabled.txt", "CoolMod/Scripts/main.lua"]
        );
        assert_eq!(
            file_by_rel(&set, "CoolMod/enabled.txt").unwrap().role,
            Role::Marker
        );
    }
}
