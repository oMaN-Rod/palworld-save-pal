-- Mod management state. Ids are caller-supplied deterministic strings, never
-- autoincrement, so a row's identity survives a reinstall and is usable as a
-- directory name. The sqlx driver enables PRAGMA foreign_keys, so these
-- references, cascades and restrictions are live: parents must exist before
-- children. The browser's OPFS driver sets no pragma, which is why ps-db also
-- deletes children explicitly, parents last.

CREATE TABLE IF NOT EXISTS mod_targets (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    server_id INTEGER REFERENCES servers(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL,
    platform TEXT NOT NULL,
    ue4ss_mode TEXT NOT NULL DEFAULT 'none',
    layout_overrides TEXT NOT NULL DEFAULT '{}',
    detected TEXT NOT NULL DEFAULT '{}',
    last_scanned_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS mod_targets_by_server ON mod_targets(server_id);

CREATE TABLE IF NOT EXISTS mods (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    custom_name TEXT,
    mod_type TEXT NOT NULL,
    author TEXT,
    summary TEXT,
    source_kind TEXT NOT NULL,
    source_ref TEXT NOT NULL DEFAULT '{}',
    nexus_mod_id INTEGER,
    ignored_version TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS mod_versions (
    id TEXT PRIMARY KEY,
    mod_id TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
    version TEXT NOT NULL,
    archive_path TEXT,
    library_dir TEXT NOT NULL,
    manifest TEXT NOT NULL,
    source_ref TEXT NOT NULL DEFAULT '{}',
    installed_at TEXT NOT NULL,
    is_current INTEGER NOT NULL DEFAULT 1,
    UNIQUE (mod_id, version)
);
CREATE INDEX IF NOT EXISTS mod_versions_by_mod ON mod_versions(mod_id);

CREATE TABLE IF NOT EXISTS profiles (
    id TEXT PRIMARY KEY,
    target_id TEXT NOT NULL REFERENCES mod_targets(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 0,
    is_default INTEGER NOT NULL DEFAULT 0,
    ue4ss_control_mode TEXT NOT NULL DEFAULT 'enabled_txt',
    force_order_ue4ss INTEGER NOT NULL DEFAULT 0,
    force_order_palschema INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS profiles_by_target ON profiles(target_id);

CREATE TABLE IF NOT EXISTS profile_mods (
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    mod_id TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
    mod_version_id TEXT REFERENCES mod_versions(id) ON DELETE RESTRICT,
    enabled INTEGER NOT NULL DEFAULT 1,
    load_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (profile_id, mod_id)
);

-- Framework selection is target state, not profile state, so switching
-- profiles never plans a framework removal.
CREATE TABLE IF NOT EXISTS target_frameworks (
    target_id TEXT NOT NULL REFERENCES mod_targets(id) ON DELETE CASCADE,
    framework TEXT NOT NULL,
    mod_version_id TEXT NOT NULL REFERENCES mod_versions(id) ON DELETE RESTRICT,
    PRIMARY KEY (target_id, framework)
);

-- The only description of what the app put on a target. `path` is absolute and
-- normalised; the primary key makes destination ownership exclusive. A shared
-- marker such as mods.txt belongs to the target rather than to one mod version,
-- which is what the CHECK enforces.
CREATE TABLE IF NOT EXISTS deployment_files (
    target_id TEXT NOT NULL REFERENCES mod_targets(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    mod_version_id TEXT REFERENCES mod_versions(id) ON DELETE RESTRICT,
    hash TEXT NOT NULL,
    role TEXT NOT NULL,
    rel_path TEXT,
    deployed_at TEXT NOT NULL,
    PRIMARY KEY (target_id, path),
    CHECK ((mod_version_id IS NULL) = (role = 'shared_marker'))
);
CREATE INDEX IF NOT EXISTS deployment_files_by_version
    ON deployment_files (target_id, mod_version_id);

-- At most one open apply per target. `op_id` also names that apply's backup set.
CREATE TABLE IF NOT EXISTS apply_journal (
    target_id TEXT PRIMARY KEY REFERENCES mod_targets(id) ON DELETE CASCADE,
    op_id TEXT NOT NULL,
    plan TEXT NOT NULL,
    started_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS world_profiles (
    world_key TEXT PRIMARY KEY,
    world_name TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    updated_at TEXT NOT NULL
);

-- SQLite has no ADD COLUMN IF NOT EXISTS, so these three are not idempotent. The
-- native path wraps each migration in a transaction; the browser path does not,
-- but it has no servers and no Amity instances to migrate.
ALTER TABLE servers ADD COLUMN paks_path TEXT NOT NULL DEFAULT '';
ALTER TABLE servers ADD COLUMN pending_relocation TEXT;

-- Which target an Amity bridge is attached to. Nullable: an instance added
-- before its target was detected has none yet.
ALTER TABLE amity_instances ADD COLUMN target_id TEXT REFERENCES mod_targets(id) ON DELETE SET NULL;
