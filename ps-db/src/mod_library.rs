use crate::error::DbError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModRow {
    pub id: String,
    pub name: String,
    pub custom_name: Option<String>,
    pub mod_type: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub source_kind: String,
    pub source_ref: String,
    pub nexus_mod_id: Option<i64>,
    pub ignored_version: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewMod {
    pub id: String,
    pub name: String,
    pub custom_name: Option<String>,
    pub mod_type: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub source_kind: String,
    pub source_ref: String,
    pub nexus_mod_id: Option<i64>,
    pub ignored_version: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModVersionRow {
    pub id: String,
    pub mod_id: String,
    pub version: String,
    pub archive_path: Option<String>,
    pub library_dir: String,
    pub manifest: String,
    pub source_ref: String,
    pub installed_at: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NewModVersion {
    pub id: String,
    pub mod_id: String,
    pub version: String,
    pub archive_path: Option<String>,
    pub library_dir: String,
    pub manifest: String,
    pub source_ref: String,
}

/// Everything that deleting a version would break. The database would refuse the
/// delete too, with a bare constraint error; computing the usage here lets the
/// caller say which targets and profiles hold the version.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersionUsage {
    pub is_current: bool,
    pub deployed_targets: Vec<String>,
    pub pinned_profiles: Vec<String>,
    pub frameworks: Vec<String>,
    /// Targets with an open apply. Coarse on purpose: the journal's plan is opaque
    /// JSON at this layer, so any open apply blocks every library deletion rather
    /// than only the versions that plan names. An apply is short; deleting a
    /// version a running apply is mid-way through installing leaves a journal
    /// nothing can finish or roll back.
    pub open_applies: Vec<String>,
}

impl VersionUsage {
    pub fn is_in_use(&self) -> bool {
        self.is_current
            || !self.deployed_targets.is_empty()
            || !self.pinned_profiles.is_empty()
            || !self.frameworks.is_empty()
            || !self.open_applies.is_empty()
    }
}

const MOD_COLUMNS: &str = "id, name, custom_name, mod_type, author, summary, source_kind, \
                           source_ref, nexus_mod_id, ignored_version, notes, created_at, updated_at";

const VERSION_COLUMNS: &str = "id, mod_id, version, archive_path, library_dir, manifest, \
                               source_ref, installed_at, is_current";

fn map_mod(r: &crate::DbRow) -> Result<ModRow, DbError> {
    Ok(ModRow {
        id: r.get_string("id")?,
        name: r.get_string("name")?,
        custom_name: r.get_opt_str("custom_name")?,
        mod_type: r.get_string("mod_type")?,
        author: r.get_opt_str("author")?,
        summary: r.get_opt_str("summary")?,
        source_kind: r.get_string("source_kind")?,
        source_ref: r.get_string("source_ref")?,
        nexus_mod_id: r.get_opt_i64("nexus_mod_id")?,
        ignored_version: r.get_opt_str("ignored_version")?,
        notes: r.get_opt_str("notes")?,
        created_at: r.get_string("created_at")?,
        updated_at: r.get_string("updated_at")?,
    })
}

fn map_version(r: &crate::DbRow) -> Result<ModVersionRow, DbError> {
    Ok(ModVersionRow {
        id: r.get_string("id")?,
        mod_id: r.get_string("mod_id")?,
        version: r.get_string("version")?,
        archive_path: r.get_opt_str("archive_path")?,
        library_dir: r.get_string("library_dir")?,
        manifest: r.get_string("manifest")?,
        source_ref: r.get_string("source_ref")?,
        installed_at: r.get_string("installed_at")?,
        is_current: r.get_bool("is_current")?,
    })
}

fn column_of(rows: &[crate::DbRow], column: &str) -> Result<Vec<String>, DbError> {
    rows.iter().map(|row| row.get_string(column)).collect()
}

async fn open_apply_targets(db: &dyn crate::DbDriver) -> Result<Vec<String>, DbError> {
    let rows = db
        .query(
            "SELECT target_id FROM apply_journal ORDER BY target_id ASC",
            &[],
        )
        .await?;
    column_of(&rows, "target_id")
}

pub async fn upsert_mod(db: &dyn crate::DbDriver, new: &NewMod) -> Result<ModRow, DbError> {
    let now = crate::time::now_iso_naive_utc();
    db.execute(
        "INSERT INTO mods (id, name, custom_name, mod_type, author, summary, source_kind, \
         source_ref, nexus_mod_id, ignored_version, notes, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12) \
         ON CONFLICT(id) DO UPDATE SET name = ?2, custom_name = ?3, mod_type = ?4, author = ?5, \
         summary = ?6, source_kind = ?7, source_ref = ?8, nexus_mod_id = ?9, \
         ignored_version = ?10, notes = ?11, updated_at = ?12",
        &[
            new.id.as_str().into(),
            new.name.as_str().into(),
            new.custom_name.clone().into(),
            new.mod_type.as_str().into(),
            new.author.clone().into(),
            new.summary.clone().into(),
            new.source_kind.as_str().into(),
            new.source_ref.as_str().into(),
            new.nexus_mod_id.into(),
            new.ignored_version.clone().into(),
            new.notes.clone().into(),
            now.as_str().into(),
        ],
    )
    .await?;
    get_mod(db, &new.id)
        .await?
        .ok_or_else(|| DbError::Other(format!("mod {} vanished after write", new.id)))
}

pub async fn get_mod(db: &dyn crate::DbDriver, id: &str) -> Result<Option<ModRow>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {MOD_COLUMNS} FROM mods WHERE id = ?1"),
            &[id.into()],
        )
        .await?;
    rows.first().map(map_mod).transpose()
}

/// False when no mod has `mod_id`.
pub async fn set_ignored_version(
    db: &dyn crate::DbDriver,
    mod_id: &str,
    version: Option<&str>,
) -> Result<bool, DbError> {
    let now = crate::time::now_iso_naive_utc();
    let changed = db
        .execute(
            "UPDATE mods SET ignored_version = ?1, updated_at = ?2 WHERE id = ?3",
            &[version.into(), now.as_str().into(), mod_id.into()],
        )
        .await?;
    Ok(changed > 0)
}

pub async fn list_mods(db: &dyn crate::DbDriver) -> Result<Vec<ModRow>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {MOD_COLUMNS} FROM mods ORDER BY name ASC, id ASC"),
            &[],
        )
        .await?;
    rows.iter().map(map_mod).collect()
}

/// The targets holding any version of this mod, whether as a deployed file or as
/// an installed framework.
pub async fn mod_usage(db: &dyn crate::DbDriver, mod_id: &str) -> Result<Vec<String>, DbError> {
    let rows = db
        .query(
            "SELECT DISTINCT target_id FROM deployment_files WHERE mod_version_id IN \
             (SELECT id FROM mod_versions WHERE mod_id = ?1) \
             UNION \
             SELECT DISTINCT target_id FROM target_frameworks WHERE mod_version_id IN \
             (SELECT id FROM mod_versions WHERE mod_id = ?1) \
             ORDER BY target_id ASC",
            &[mod_id.into()],
        )
        .await?;
    column_of(&rows, "target_id")
}

/// Refuses while any target still holds the mod, then deletes its versions and
/// profile entries explicitly, because the browser's driver sets no foreign-key
/// pragma and would otherwise orphan them.
pub async fn remove_mod(db: &dyn crate::DbDriver, id: &str) -> Result<bool, DbError> {
    let holders = mod_usage(db, id).await?;
    if !holders.is_empty() {
        return Err(DbError::Other(format!(
            "mod {id} is still deployed on {holders:?}"
        )));
    }
    let applying = open_apply_targets(db).await?;
    if !applying.is_empty() {
        return Err(DbError::Other(format!(
            "mod {id} cannot be removed while an apply is open on {applying:?}"
        )));
    }
    db.execute_batch(&[
        (
            "DELETE FROM profile_mods WHERE mod_id = ?1",
            vec![id.into()],
        ),
        (
            "DELETE FROM mod_versions WHERE mod_id = ?1",
            vec![id.into()],
        ),
    ])
    .await?;
    let affected = db
        .execute("DELETE FROM mods WHERE id = ?1", &[id.into()])
        .await?;
    Ok(affected > 0)
}

/// The first version of a mod becomes current; a later one does not, so
/// installing an update never redirects a target that is deployed with the
/// version it already had.
pub async fn insert_version(
    db: &dyn crate::DbDriver,
    new: &NewModVersion,
) -> Result<ModVersionRow, DbError> {
    let is_current = versions_of(db, &new.mod_id).await?.is_empty();
    db.execute(
        "INSERT INTO mod_versions (id, mod_id, version, archive_path, library_dir, manifest, \
         source_ref, installed_at, is_current) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT(id) DO UPDATE SET archive_path = ?4, library_dir = ?5, manifest = ?6, \
         source_ref = ?7",
        &[
            new.id.as_str().into(),
            new.mod_id.as_str().into(),
            new.version.as_str().into(),
            new.archive_path.clone().into(),
            new.library_dir.as_str().into(),
            new.manifest.as_str().into(),
            new.source_ref.as_str().into(),
            crate::time::now_iso_naive_utc().as_str().into(),
            is_current.into(),
        ],
    )
    .await?;
    get_version(db, &new.id)
        .await?
        .ok_or_else(|| DbError::Other(format!("mod version {} vanished after write", new.id)))
}

pub async fn get_version(
    db: &dyn crate::DbDriver,
    id: &str,
) -> Result<Option<ModVersionRow>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {VERSION_COLUMNS} FROM mod_versions WHERE id = ?1"),
            &[id.into()],
        )
        .await?;
    rows.first().map(map_version).transpose()
}

pub async fn versions_of(
    db: &dyn crate::DbDriver,
    mod_id: &str,
) -> Result<Vec<ModVersionRow>, DbError> {
    let rows = db
        .query(
            &format!(
                "SELECT {VERSION_COLUMNS} FROM mod_versions WHERE mod_id = ?1 \
                 ORDER BY installed_at ASC, id ASC"
            ),
            &[mod_id.into()],
        )
        .await?;
    rows.iter().map(map_version).collect()
}

pub async fn current_version(
    db: &dyn crate::DbDriver,
    mod_id: &str,
) -> Result<Option<ModVersionRow>, DbError> {
    let rows = db
        .query(
            &format!(
                "SELECT {VERSION_COLUMNS} FROM mod_versions WHERE mod_id = ?1 AND is_current = 1"
            ),
            &[mod_id.into()],
        )
        .await?;
    rows.first().map(map_version).transpose()
}

/// Validates before it clears: without the check, a version id that matches no row
/// would clear every `is_current` flag and set none, leaving the mod with no
/// current version and no error.
pub async fn set_current_version(
    db: &dyn crate::DbDriver,
    mod_id: &str,
    version_id: &str,
) -> Result<(), DbError> {
    match get_version(db, version_id).await? {
        None => {
            return Err(DbError::Other(format!(
                "mod version {version_id} not found"
            )))
        }
        Some(version) if version.mod_id != mod_id => {
            return Err(DbError::Other(format!(
                "mod version {version_id} belongs to mod {}, not {mod_id}",
                version.mod_id
            )))
        }
        Some(_) => {}
    }
    db.execute_batch(&[
        (
            "UPDATE mod_versions SET is_current = 0 WHERE mod_id = ?1",
            vec![mod_id.into()],
        ),
        (
            "UPDATE mod_versions SET is_current = 1 WHERE id = ?1 AND mod_id = ?2",
            vec![version_id.into(), mod_id.into()],
        ),
    ])
    .await?;
    Ok(())
}

pub async fn version_usage(
    db: &dyn crate::DbDriver,
    version_id: &str,
) -> Result<VersionUsage, DbError> {
    let current = db
        .query(
            "SELECT id FROM mod_versions WHERE id = ?1 AND is_current = 1",
            &[version_id.into()],
        )
        .await?;
    let deployed = db
        .query(
            "SELECT DISTINCT target_id FROM deployment_files WHERE mod_version_id = ?1 \
             ORDER BY target_id ASC",
            &[version_id.into()],
        )
        .await?;
    let pinned = db
        .query(
            "SELECT DISTINCT profile_id FROM profile_mods WHERE mod_version_id = ?1 \
             ORDER BY profile_id ASC",
            &[version_id.into()],
        )
        .await?;
    let frameworks = db
        .query(
            "SELECT DISTINCT target_id FROM target_frameworks WHERE mod_version_id = ?1 \
             ORDER BY target_id ASC",
            &[version_id.into()],
        )
        .await?;
    Ok(VersionUsage {
        is_current: !current.is_empty(),
        deployed_targets: column_of(&deployed, "target_id")?,
        pinned_profiles: column_of(&pinned, "profile_id")?,
        frameworks: column_of(&frameworks, "target_id")?,
        open_applies: open_apply_targets(db).await?,
    })
}

pub async fn remove_version(db: &dyn crate::DbDriver, version_id: &str) -> Result<(), DbError> {
    let usage = version_usage(db, version_id).await?;
    if usage.is_in_use() {
        return Err(DbError::Other(format!(
            "mod version {version_id} is in use: current={}, deployed on {:?}, pinned by {:?}, \
             installed as a framework on {:?}, apply open on {:?}",
            usage.is_current,
            usage.deployed_targets,
            usage.pinned_profiles,
            usage.frameworks,
            usage.open_applies
        )));
    }
    db.execute(
        "DELETE FROM mod_versions WHERE id = ?1",
        &[version_id.into()],
    )
    .await?;
    Ok(())
}
