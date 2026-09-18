use crate::error::DbError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentFile {
    pub target_id: String,
    pub path: String,
    /// NULL exactly when `role` is `shared_marker`, which the table's CHECK enforces.
    pub mod_version_id: Option<String>,
    pub hash: String,
    pub role: String,
    pub rel_path: Option<String>,
    pub deployed_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewDeploymentFile {
    pub target_id: String,
    pub path: String,
    pub mod_version_id: Option<String>,
    pub hash: String,
    pub role: String,
    pub rel_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyJournal {
    pub target_id: String,
    pub op_id: String,
    /// JSON, opaque here: the plan with the hash each path is expected to end at.
    pub plan: String,
    pub started_at: String,
}

const FILE_COLUMNS: &str = "target_id, path, mod_version_id, hash, role, rel_path, deployed_at";

const JOURNAL_COLUMNS: &str = "target_id, op_id, plan, started_at";

const RECORD_SQL: &str =
    "INSERT INTO deployment_files (target_id, path, mod_version_id, hash, role, rel_path, \
     deployed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
     ON CONFLICT(target_id, path) DO UPDATE SET mod_version_id = ?3, hash = ?4, role = ?5, \
     rel_path = ?6, deployed_at = ?7";

fn map_file(r: &crate::DbRow) -> Result<DeploymentFile, DbError> {
    Ok(DeploymentFile {
        target_id: r.get_string("target_id")?,
        path: r.get_string("path")?,
        mod_version_id: r.get_opt_str("mod_version_id")?,
        hash: r.get_string("hash")?,
        role: r.get_string("role")?,
        rel_path: r.get_opt_str("rel_path")?,
        deployed_at: r.get_string("deployed_at")?,
    })
}

fn map_journal(r: &crate::DbRow) -> Result<ApplyJournal, DbError> {
    Ok(ApplyJournal {
        target_id: r.get_string("target_id")?,
        op_id: r.get_string("op_id")?,
        plan: r.get_string("plan")?,
        started_at: r.get_string("started_at")?,
    })
}

/// Records a whole applied chunk as one transaction. A partially recorded chunk
/// would make the next apply plan against a state that never existed on disk.
pub async fn record(db: &dyn crate::DbDriver, files: &[NewDeploymentFile]) -> Result<(), DbError> {
    if files.is_empty() {
        return Ok(());
    }
    let now = crate::time::now_iso_naive_utc();
    let statements: Vec<(&str, Vec<crate::DbValue>)> = files
        .iter()
        .map(|f| {
            (
                RECORD_SQL,
                vec![
                    f.target_id.as_str().into(),
                    f.path.as_str().into(),
                    f.mod_version_id.clone().into(),
                    f.hash.as_str().into(),
                    f.role.as_str().into(),
                    f.rel_path.clone().into(),
                    now.as_str().into(),
                ],
            )
        })
        .collect();
    db.execute_batch(&statements).await
}

pub async fn forget(
    db: &dyn crate::DbDriver,
    target_id: &str,
    paths: &[&str],
) -> Result<(), DbError> {
    if paths.is_empty() {
        return Ok(());
    }
    const SQL: &str = "DELETE FROM deployment_files WHERE target_id = ?1 AND path = ?2";
    let statements: Vec<(&str, Vec<crate::DbValue>)> = paths
        .iter()
        .map(|path| (SQL, vec![target_id.into(), (*path).into()]))
        .collect();
    db.execute_batch(&statements).await
}

/// The targets whose `deployment_files` reference any version of this mod.
/// Unlike `mod_library::mod_usage`, a `target_frameworks` slot never counts:
/// a framework is installed on the target, not deployed as this mod's files.
pub async fn deployed_targets_of(
    db: &dyn crate::DbDriver,
    mod_id: &str,
) -> Result<Vec<String>, DbError> {
    let rows = db
        .query(
            "SELECT DISTINCT target_id FROM deployment_files WHERE mod_version_id IN \
             (SELECT id FROM mod_versions WHERE mod_id = ?1) ORDER BY target_id ASC",
            &[mod_id.into()],
        )
        .await?;
    rows.iter().map(|r| r.get_string("target_id")).collect()
}

pub async fn files_of(
    db: &dyn crate::DbDriver,
    target_id: &str,
) -> Result<Vec<DeploymentFile>, DbError> {
    let rows = db
        .query(
            &format!(
                "SELECT {FILE_COLUMNS} FROM deployment_files WHERE target_id = ?1 \
                 ORDER BY path ASC"
            ),
            &[target_id.into()],
        )
        .await?;
    rows.iter().map(map_file).collect()
}

pub async fn file_at(
    db: &dyn crate::DbDriver,
    target_id: &str,
    path: &str,
) -> Result<Option<DeploymentFile>, DbError> {
    let rows = db
        .query(
            &format!(
                "SELECT {FILE_COLUMNS} FROM deployment_files WHERE target_id = ?1 AND path = ?2"
            ),
            &[target_id.into(), path.into()],
        )
        .await?;
    rows.first().map(map_file).transpose()
}

pub async fn files_for_version(
    db: &dyn crate::DbDriver,
    target_id: &str,
    mod_version_id: &str,
) -> Result<Vec<DeploymentFile>, DbError> {
    let rows = db
        .query(
            &format!(
                "SELECT {FILE_COLUMNS} FROM deployment_files \
                 WHERE target_id = ?1 AND mod_version_id = ?2 ORDER BY path ASC"
            ),
            &[target_id.into(), mod_version_id.into()],
        )
        .await?;
    rows.iter().map(map_file).collect()
}

pub async fn shared_markers(
    db: &dyn crate::DbDriver,
    target_id: &str,
) -> Result<Vec<DeploymentFile>, DbError> {
    let rows = db
        .query(
            &format!(
                "SELECT {FILE_COLUMNS} FROM deployment_files \
                 WHERE target_id = ?1 AND mod_version_id IS NULL ORDER BY path ASC"
            ),
            &[target_id.into()],
        )
        .await?;
    rows.iter().map(map_file).collect()
}

/// Refuses while a journal is already open for the target. One open journal per
/// target is what makes recovery possible; a second open would overwrite the
/// plan the recovery path reads.
pub async fn open_journal(
    db: &dyn crate::DbDriver,
    target_id: &str,
    op_id: &str,
    plan: &str,
) -> Result<ApplyJournal, DbError> {
    if let Some(existing) = journal_of(db, target_id).await? {
        return Err(DbError::Other(format!(
            "target {target_id} already has an open apply {} started at {}",
            existing.op_id, existing.started_at
        )));
    }
    db.execute(
        "INSERT INTO apply_journal (target_id, op_id, plan, started_at) VALUES (?1, ?2, ?3, ?4)",
        &[
            target_id.into(),
            op_id.into(),
            plan.into(),
            crate::time::now_iso_naive_utc().as_str().into(),
        ],
    )
    .await?;
    journal_of(db, target_id)
        .await?
        .ok_or_else(|| DbError::Other(format!("apply journal {target_id} vanished after write")))
}

pub async fn journal_of(
    db: &dyn crate::DbDriver,
    target_id: &str,
) -> Result<Option<ApplyJournal>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {JOURNAL_COLUMNS} FROM apply_journal WHERE target_id = ?1"),
            &[target_id.into()],
        )
        .await?;
    rows.first().map(map_journal).transpose()
}

/// Every apply left open by a crash. The startup recovery sweep reads this.
pub async fn open_journals(db: &dyn crate::DbDriver) -> Result<Vec<ApplyJournal>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {JOURNAL_COLUMNS} FROM apply_journal ORDER BY target_id ASC"),
            &[],
        )
        .await?;
    rows.iter().map(map_journal).collect()
}

pub async fn close_journal(db: &dyn crate::DbDriver, target_id: &str) -> Result<bool, DbError> {
    let affected = db
        .execute(
            "DELETE FROM apply_journal WHERE target_id = ?1",
            &[target_id.into()],
        )
        .await?;
    Ok(affected > 0)
}
