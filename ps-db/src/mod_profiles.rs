use crate::error::DbError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRow {
    pub id: String,
    pub target_id: String,
    pub name: String,
    pub is_active: bool,
    pub is_default: bool,
    pub ue4ss_control_mode: String,
    pub force_order_ue4ss: bool,
    pub force_order_palschema: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewProfile {
    pub id: String,
    pub target_id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileModRow {
    pub profile_id: String,
    pub mod_id: String,
    /// NULL means "follow the mod's current version".
    pub mod_version_id: Option<String>,
    pub enabled: bool,
    pub load_order: i64,
}

const PROFILE_COLUMNS: &str = "id, target_id, name, is_active, is_default, ue4ss_control_mode, \
                               force_order_ue4ss, force_order_palschema, created_at, updated_at";

const ENTRY_COLUMNS: &str = "profile_id, mod_id, mod_version_id, enabled, load_order";

fn map_profile(r: &crate::DbRow) -> Result<ProfileRow, DbError> {
    Ok(ProfileRow {
        id: r.get_string("id")?,
        target_id: r.get_string("target_id")?,
        name: r.get_string("name")?,
        is_active: r.get_bool("is_active")?,
        is_default: r.get_bool("is_default")?,
        ue4ss_control_mode: r.get_string("ue4ss_control_mode")?,
        force_order_ue4ss: r.get_bool("force_order_ue4ss")?,
        force_order_palschema: r.get_bool("force_order_palschema")?,
        created_at: r.get_string("created_at")?,
        updated_at: r.get_string("updated_at")?,
    })
}

fn map_entry(r: &crate::DbRow) -> Result<ProfileModRow, DbError> {
    Ok(ProfileModRow {
        profile_id: r.get_string("profile_id")?,
        mod_id: r.get_string("mod_id")?,
        mod_version_id: r.get_opt_str("mod_version_id")?,
        enabled: r.get_bool("enabled")?,
        load_order: r.get_i64("load_order")?,
    })
}

async fn require(db: &dyn crate::DbDriver, id: &str) -> Result<ProfileRow, DbError> {
    get(db, id)
        .await?
        .ok_or_else(|| DbError::Other(format!("profile {id} not found")))
}

/// A new profile is active only when its target has none yet, so creating a
/// profile never silently switches what is deployed.
pub async fn create(db: &dyn crate::DbDriver, new: &NewProfile) -> Result<ProfileRow, DbError> {
    let is_active = active_for_target(db, &new.target_id).await?.is_none();
    let now = crate::time::now_iso_naive_utc();
    db.execute(
        "INSERT INTO profiles (id, target_id, name, is_active, is_default, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        &[
            new.id.as_str().into(),
            new.target_id.as_str().into(),
            new.name.as_str().into(),
            is_active.into(),
            new.is_default.into(),
            now.as_str().into(),
        ],
    )
    .await?;
    require(db, &new.id).await
}

pub async fn get(db: &dyn crate::DbDriver, id: &str) -> Result<Option<ProfileRow>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {PROFILE_COLUMNS} FROM profiles WHERE id = ?1"),
            &[id.into()],
        )
        .await?;
    rows.first().map(map_profile).transpose()
}

pub async fn for_target(
    db: &dyn crate::DbDriver,
    target_id: &str,
) -> Result<Vec<ProfileRow>, DbError> {
    let rows = db
        .query(
            &format!(
                "SELECT {PROFILE_COLUMNS} FROM profiles WHERE target_id = ?1 \
                 ORDER BY is_default DESC, name ASC, id ASC"
            ),
            &[target_id.into()],
        )
        .await?;
    rows.iter().map(map_profile).collect()
}

async fn one_flagged(
    db: &dyn crate::DbDriver,
    target_id: &str,
    column: &str,
) -> Result<Option<ProfileRow>, DbError> {
    let rows = db
        .query(
            &format!(
                "SELECT {PROFILE_COLUMNS} FROM profiles WHERE target_id = ?1 AND {column} = 1"
            ),
            &[target_id.into()],
        )
        .await?;
    rows.first().map(map_profile).transpose()
}

pub async fn active_for_target(
    db: &dyn crate::DbDriver,
    target_id: &str,
) -> Result<Option<ProfileRow>, DbError> {
    one_flagged(db, target_id, "is_active").await
}

pub async fn default_for_target(
    db: &dyn crate::DbDriver,
    target_id: &str,
) -> Result<Option<ProfileRow>, DbError> {
    one_flagged(db, target_id, "is_default").await
}

pub async fn rename(db: &dyn crate::DbDriver, id: &str, name: &str) -> Result<(), DbError> {
    db.execute(
        "UPDATE profiles SET name = ?1, updated_at = ?2 WHERE id = ?3",
        &[
            name.into(),
            crate::time::now_iso_naive_utc().as_str().into(),
            id.into(),
        ],
    )
    .await?;
    Ok(())
}

pub async fn set_options(
    db: &dyn crate::DbDriver,
    id: &str,
    control_mode: &str,
    force_ue4ss: bool,
    force_palschema: bool,
) -> Result<(), DbError> {
    db.execute(
        "UPDATE profiles SET ue4ss_control_mode = ?1, force_order_ue4ss = ?2, \
         force_order_palschema = ?3, updated_at = ?4 WHERE id = ?5",
        &[
            control_mode.into(),
            force_ue4ss.into(),
            force_palschema.into(),
            crate::time::now_iso_naive_utc().as_str().into(),
            id.into(),
        ],
    )
    .await?;
    Ok(())
}

/// Clears the flag across the target's profiles and sets it on one of them,
/// returning the profile that is now active.
pub async fn activate(
    db: &dyn crate::DbDriver,
    target_id: &str,
    id: &str,
) -> Result<ProfileRow, DbError> {
    let profile = require(db, id).await?;
    if profile.target_id != target_id {
        return Err(DbError::Other(format!(
            "profile {id} belongs to target {}, not {target_id}",
            profile.target_id
        )));
    }
    let now = crate::time::now_iso_naive_utc();
    db.execute_batch(&[
        (
            "UPDATE profiles SET is_active = 0, updated_at = ?1 WHERE target_id = ?2 \
             AND is_active = 1",
            vec![now.as_str().into(), target_id.into()],
        ),
        (
            "UPDATE profiles SET is_active = 1, updated_at = ?1 WHERE id = ?2",
            vec![now.as_str().into(), id.into()],
        ),
    ])
    .await?;
    require(db, id).await
}

/// Refuses the default profile, unlinks any worlds pointing at this one, and
/// returns whichever profile is active once the deletion is done, which the
/// caller applies.
pub async fn delete(db: &dyn crate::DbDriver, id: &str) -> Result<ProfileRow, DbError> {
    let profile = require(db, id).await?;
    if profile.is_default {
        return Err(DbError::Other(format!(
            "profile {id} is the default profile of target {} and cannot be deleted",
            profile.target_id
        )));
    }
    let fallback = default_for_target(db, &profile.target_id)
        .await?
        .ok_or_else(|| {
            DbError::Other(format!(
                "target {} has no default profile",
                profile.target_id
            ))
        })?;
    let now = crate::time::now_iso_naive_utc();
    let mut statements: Vec<(&str, Vec<crate::DbValue>)> = vec![
        (
            "DELETE FROM world_profiles WHERE profile_id = ?1",
            vec![id.into()],
        ),
        (
            "DELETE FROM profile_mods WHERE profile_id = ?1",
            vec![id.into()],
        ),
        ("DELETE FROM profiles WHERE id = ?1", vec![id.into()]),
    ];
    if profile.is_active {
        statements.push((
            "UPDATE profiles SET is_active = 1, updated_at = ?1 WHERE id = ?2",
            vec![now.as_str().into(), fallback.id.as_str().into()],
        ));
    }
    db.execute_batch(&statements).await?;
    active_for_target(db, &profile.target_id)
        .await?
        .ok_or_else(|| {
            DbError::Other(format!(
                "target {} has no active profile",
                profile.target_id
            ))
        })
}

/// Upserts one entry. The mod must exist, and a pinned version must exist and
/// belong to that mod: nothing downstream re-checks, and a foreign pin would
/// only surface when the deployer tried to resolve it.
pub async fn set_mod(db: &dyn crate::DbDriver, entry: &ProfileModRow) -> Result<(), DbError> {
    if crate::mod_library::get_mod(db, &entry.mod_id)
        .await?
        .is_none()
    {
        return Err(DbError::Other(format!("mod {} not found", entry.mod_id)));
    }
    if let Some(version_id) = entry.mod_version_id.as_deref() {
        match crate::mod_library::get_version(db, version_id).await? {
            None => {
                return Err(DbError::Other(format!(
                    "mod version {version_id} not found"
                )))
            }
            Some(version) if version.mod_id != entry.mod_id => {
                return Err(DbError::Other(format!(
                    "mod version {version_id} belongs to mod {}, not {}",
                    version.mod_id, entry.mod_id
                )))
            }
            Some(_) => {}
        }
    }
    db.execute(
        "INSERT INTO profile_mods (profile_id, mod_id, mod_version_id, enabled, load_order) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(profile_id, mod_id) DO UPDATE SET mod_version_id = ?3, enabled = ?4, \
         load_order = ?5",
        &[
            entry.profile_id.as_str().into(),
            entry.mod_id.as_str().into(),
            entry.mod_version_id.clone().into(),
            entry.enabled.into(),
            entry.load_order.into(),
        ],
    )
    .await?;
    Ok(())
}

pub async fn unset_mod(
    db: &dyn crate::DbDriver,
    profile_id: &str,
    mod_id: &str,
) -> Result<bool, DbError> {
    let affected = db
        .execute(
            "DELETE FROM profile_mods WHERE profile_id = ?1 AND mod_id = ?2",
            &[profile_id.into(), mod_id.into()],
        )
        .await?;
    Ok(affected > 0)
}

/// Deletes the entry only while it is still disabled, so an entry re-enabled
/// between a caller's read and this delete is left alone rather than removed
/// out from under a user who just turned it back on.
pub async fn unset_if_disabled(
    db: &dyn crate::DbDriver,
    profile_id: &str,
    mod_id: &str,
) -> Result<bool, DbError> {
    let affected = db
        .execute(
            "DELETE FROM profile_mods WHERE profile_id = ?1 AND mod_id = ?2 AND enabled = 0",
            &[profile_id.into(), mod_id.into()],
        )
        .await?;
    Ok(affected > 0)
}

pub async fn mods_of(
    db: &dyn crate::DbDriver,
    profile_id: &str,
) -> Result<Vec<ProfileModRow>, DbError> {
    let rows = db
        .query(
            &format!(
                "SELECT {ENTRY_COLUMNS} FROM profile_mods WHERE profile_id = ?1 \
                 ORDER BY load_order ASC, mod_id ASC"
            ),
            &[profile_id.into()],
        )
        .await?;
    rows.iter().map(map_entry).collect()
}

/// Renumbers the listed mods from zero in the order given. A mod absent from the
/// list keeps its current `load_order`, so a per-kind reorder does not disturb
/// the other kinds.
pub async fn set_load_order(
    db: &dyn crate::DbDriver,
    profile_id: &str,
    ordered_mod_ids: &[&str],
) -> Result<(), DbError> {
    const SQL: &str =
        "UPDATE profile_mods SET load_order = ?1 WHERE profile_id = ?2 AND mod_id = ?3";
    let statements: Vec<(&str, Vec<crate::DbValue>)> = ordered_mod_ids
        .iter()
        .enumerate()
        .map(|(index, mod_id)| {
            (
                SQL,
                vec![(index as i64).into(), profile_id.into(), (*mod_id).into()],
            )
        })
        .collect();
    db.execute_batch(&statements).await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFramework {
    pub target_id: String,
    pub framework: String,
    pub mod_version_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldProfile {
    pub world_key: String,
    pub world_name: String,
    pub profile_id: String,
    pub updated_at: String,
}

fn map_framework(r: &crate::DbRow) -> Result<TargetFramework, DbError> {
    Ok(TargetFramework {
        target_id: r.get_string("target_id")?,
        framework: r.get_string("framework")?,
        mod_version_id: r.get_string("mod_version_id")?,
    })
}

fn map_world(r: &crate::DbRow) -> Result<WorldProfile, DbError> {
    Ok(WorldProfile {
        world_key: r.get_string("world_key")?,
        world_name: r.get_string("world_name")?,
        profile_id: r.get_string("profile_id")?,
        updated_at: r.get_string("updated_at")?,
    })
}

/// Points one framework slot at one library version, replacing whatever the slot
/// held. The version must exist and belong to a mod of type `framework`: a slot
/// pointing at a gameplay mod would have the deployer install it into the
/// framework's directory.
pub async fn set_framework(
    db: &dyn crate::DbDriver,
    target_id: &str,
    framework: &str,
    mod_version_id: &str,
) -> Result<(), DbError> {
    let version = crate::mod_library::get_version(db, mod_version_id)
        .await?
        .ok_or_else(|| DbError::Other(format!("mod version {mod_version_id} not found")))?;
    let owner = crate::mod_library::get_mod(db, &version.mod_id)
        .await?
        .ok_or_else(|| DbError::Other(format!("mod {} not found", version.mod_id)))?;
    if owner.mod_type != "framework" {
        return Err(DbError::Other(format!(
            "mod {} is of type {}, not framework",
            owner.id, owner.mod_type
        )));
    }
    db.execute(
        "INSERT INTO target_frameworks (target_id, framework, mod_version_id) \
         VALUES (?1, ?2, ?3) \
         ON CONFLICT(target_id, framework) DO UPDATE SET mod_version_id = ?3",
        &[target_id.into(), framework.into(), mod_version_id.into()],
    )
    .await?;
    Ok(())
}

pub async fn framework_of(
    db: &dyn crate::DbDriver,
    target_id: &str,
    framework: &str,
) -> Result<Option<TargetFramework>, DbError> {
    let rows = db
        .query(
            "SELECT target_id, framework, mod_version_id FROM target_frameworks \
             WHERE target_id = ?1 AND framework = ?2",
            &[target_id.into(), framework.into()],
        )
        .await?;
    rows.first().map(map_framework).transpose()
}

pub async fn frameworks_of(
    db: &dyn crate::DbDriver,
    target_id: &str,
) -> Result<Vec<TargetFramework>, DbError> {
    let rows = db
        .query(
            "SELECT target_id, framework, mod_version_id FROM target_frameworks \
             WHERE target_id = ?1 ORDER BY framework ASC",
            &[target_id.into()],
        )
        .await?;
    rows.iter().map(map_framework).collect()
}

/// The targets whose framework slots hold any version of this mod.
pub async fn framework_targets_of(
    db: &dyn crate::DbDriver,
    mod_id: &str,
) -> Result<Vec<String>, DbError> {
    let rows = db
        .query(
            "SELECT DISTINCT target_id FROM target_frameworks WHERE mod_version_id IN \
             (SELECT id FROM mod_versions WHERE mod_id = ?1) ORDER BY target_id ASC",
            &[mod_id.into()],
        )
        .await?;
    rows.iter().map(|r| r.get_string("target_id")).collect()
}

pub async fn remove_framework(
    db: &dyn crate::DbDriver,
    target_id: &str,
    framework: &str,
) -> Result<bool, DbError> {
    let affected = db
        .execute(
            "DELETE FROM target_frameworks WHERE target_id = ?1 AND framework = ?2",
            &[target_id.into(), framework.into()],
        )
        .await?;
    Ok(affected > 0)
}

/// Links a save directory to a profile, replacing any existing link for that
/// world. Checking the profile here turns a bare constraint error into a message
/// naming the missing profile.
pub async fn link_world(
    db: &dyn crate::DbDriver,
    world_key: &str,
    world_name: &str,
    profile_id: &str,
) -> Result<WorldProfile, DbError> {
    require(db, profile_id).await?;
    let now = crate::time::now_iso_naive_utc();
    db.execute(
        "INSERT INTO world_profiles (world_key, world_name, profile_id, updated_at) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(world_key) DO UPDATE SET world_name = ?2, profile_id = ?3, updated_at = ?4",
        &[
            world_key.into(),
            world_name.into(),
            profile_id.into(),
            now.as_str().into(),
        ],
    )
    .await?;
    world_link(db, world_key)
        .await?
        .ok_or_else(|| DbError::Other(format!("world link {world_key} vanished after write")))
}

pub async fn unlink_world(db: &dyn crate::DbDriver, world_key: &str) -> Result<bool, DbError> {
    let affected = db
        .execute(
            "DELETE FROM world_profiles WHERE world_key = ?1",
            &[world_key.into()],
        )
        .await?;
    Ok(affected > 0)
}

pub async fn world_link(
    db: &dyn crate::DbDriver,
    world_key: &str,
) -> Result<Option<WorldProfile>, DbError> {
    let rows = db
        .query(
            "SELECT world_key, world_name, profile_id, updated_at FROM world_profiles \
             WHERE world_key = ?1",
            &[world_key.into()],
        )
        .await?;
    rows.first().map(map_world).transpose()
}

pub async fn list_world_links(db: &dyn crate::DbDriver) -> Result<Vec<WorldProfile>, DbError> {
    let rows = db
        .query(
            "SELECT world_key, world_name, profile_id, updated_at FROM world_profiles \
             ORDER BY world_name ASC, world_key ASC",
            &[],
        )
        .await?;
    rows.iter().map(map_world).collect()
}

pub async fn worlds_for_profile(
    db: &dyn crate::DbDriver,
    profile_id: &str,
) -> Result<Vec<WorldProfile>, DbError> {
    let rows = db
        .query(
            "SELECT world_key, world_name, profile_id, updated_at FROM world_profiles \
             WHERE profile_id = ?1 ORDER BY world_name ASC, world_key ASC",
            &[profile_id.into()],
        )
        .await?;
    rows.iter().map(map_world).collect()
}
