//! Mod management engine: archive analysis, routing manifests, target
//! layouts, marker-file codecs, and the deployment diff.
//!
//! Every function here is pure: text or lists in, structures out. Nothing
//! reads the filesystem, the network, or the environment, so the module
//! compiles for wasm and is tested entirely from fixtures. `ps-server` owns
//! the impure half (extraction, deployment, scanning).

pub mod analyzer;
pub mod conflicts;
pub mod frameworks;
pub mod ids;
pub mod layout;
pub mod manifest;
pub mod modinfo;
pub mod mods_txt;
pub mod naming;
pub mod pak_index;
pub mod palmodsettings;
pub mod paths;
pub mod plan;
pub mod types;
pub mod version;

pub use analyzer::{analyze_flags, detect_mod_type, file_paths, ArchiveFlags};
pub use conflicts::{
    group_overlaps, iostore_sibling, is_pak_route, palschema_row_keys, required_frameworks,
    strip_jsonc, workshop_dependency, Dependency, PakOverlap, PakRef, PakSource,
};
pub use frameworks::{
    framework_package, FrameworkArchiveError, FrameworkKey, FrameworkPackage,
    PALSCHEMA_VERSION_FILE, UE4SS_VERSION_FILE,
};
pub use ids::{mod_id, slugify, target_id, ModIdInput};
pub use layout::{resolve_layout, LayoutError, LayoutOverrides, TargetLayout, TargetSpec};
pub use manifest::{build_manifest, AnalyzeInput};
pub use modinfo::{
    parse_modinfo, parse_workshop_info, route_kind_for_rule, InstallRule, ModInfoJson,
    ModInfoRoute, WorkshopInfo,
};
pub use mods_txt::{ModsTxt, ModsTxtLine, UE4SS_DEFAULT_MODS_TXT};
pub use naming::{
    clean_archive_name, detect_folder_name, is_forbidden_name, nexus_ids_from_archive_name,
};
pub use pak_index::{asset_key, read_pak_listing, PakIndexError, PakListing};
pub use palmodsettings::PalModSettings;
pub use paths::{backup_key, native_separators, normalize_physical_path};
pub use plan::{
    build_plan, DeployPlan, DesiredFile, DiskState, JournalHints, PlanEntry, PreflightError,
    RecordedFile,
};
pub use types::{
    ArchiveEntry, Decision, FileRoute, InstallManifest, ModType, Platform, Role, RouteKind,
    SourceHint, TargetKind, Ue4ssMode,
};
pub use version::{compare_versions, is_newer};
