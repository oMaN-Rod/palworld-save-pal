//! Cross-player UID swap: exchanges two players' UIDs everywhere they appear
//! in `Level.sav`, their own `.sav` files, and the on-disk file names.

use crate::domain::guild_tail;
use crate::domain::{player, world};
use crate::error::CoreError;
use crate::progress::ProgressSink;
use crate::props;
use crate::session::SaveSession;
use crate::transfer::{self, TransferError};
use uuid::Uuid;

/// Every property name across the save tree that holds a player UID.
const OWNERSHIP_KEYS: [&str; 4] = [
    "OwnerPlayerUId",
    "owner_player_uid",
    "build_player_uid",
    "private_lock_player_uid",
];

/// Retargets both players' UIDs in every guild: character handles (matched by
/// character instance id), the admin uid, and each member's `player_uid`.
/// Every swap is bidirectional -- old->new AND new->old in the same pass.
fn swap_guild_member_uids(
    level: &mut crate::ue::Save,
    old_uid: Uuid,
    new_uid: Uuid,
    old_instance_id: Uuid,
    new_instance_id: Uuid,
) -> Result<(), CoreError> {
    for entry in world::group_map_mut(level)?.iter_mut() {
        if guild_tail::entry_group_type(entry).as_deref() != Some("EPalGroupType::Guild") {
            continue;
        }
        let Some(group_data) = guild_tail::entry_group_data_mut(entry) else {
            continue;
        };

        for handle in group_data.individual_character_handle_ids.iter_mut() {
            let handle_instance_id = props::guid_to_uuid(&handle.instance_id);
            if handle_instance_id == old_instance_id {
                handle.guid = props::uuid_to_guid(new_uid);
            } else if handle_instance_id == new_instance_id {
                handle.guid = props::uuid_to_guid(old_uid);
            }
        }

        let Some(guild) = guild_tail::as_guild_mut(group_data) else {
            continue;
        };
        guild_tail::swap_player_uids(guild, old_uid, new_uid);
    }
    Ok(())
}

impl SaveSession {
    /// `TransferError::Rejected` is a SOFT rejection: the handler reports it
    /// as an `{"error": ...}` payload on the normal response, not as a WS
    /// error frame.
    pub fn swap_player_uids(
        &mut self,
        old_player_uid: Uuid,
        new_player_uid: Uuid,
        progress: &ProgressSink,
    ) -> Result<(), TransferError> {
        if old_player_uid == new_player_uid {
            return Err(TransferError::Rejected("Both players are the same.".into()));
        }

        progress("Validating players...");
        for uid in [old_player_uid, new_player_uid] {
            if !self.player_file_refs.contains_key(&uid) {
                return Err(TransferError::Rejected(format!("Player {uid} not found.")));
            }
            transfer::ensure_player_gvas_loaded(self, uid)?;
        }

        let old_instance_id = self
            .loaded_players
            .get(&old_player_uid)
            .and_then(|loaded| player::save_data_props(&loaded.sav).ok())
            .and_then(transfer::save_data_instance_id)
            .ok_or_else(|| {
                TransferError::Rejected("Source player SaveData is missing or invalid.".into())
            })?;
        let new_instance_id = self
            .loaded_players
            .get(&new_player_uid)
            .and_then(|loaded| player::save_data_props(&loaded.sav).ok())
            .and_then(transfer::save_data_instance_id)
            .ok_or_else(|| {
                TransferError::Rejected("Target player SaveData is missing or invalid.".into())
            })?;

        progress("Swapping player UIDs in save data...");
        self.swap_player_gvas_uids(old_player_uid, new_player_uid);

        progress("Swapping UIDs in character save parameter map...");
        for entry in world::character_map_mut(&mut self.level)?.iter_mut() {
            let Some(instance_id) = world::entry_instance_id(entry) else {
                continue;
            };
            if instance_id == old_instance_id {
                world::set_entry_player_uid(entry, new_player_uid);
            } else if instance_id == new_instance_id {
                world::set_entry_player_uid(entry, old_player_uid);
            }
        }

        progress("Swapping UIDs in guild data...");
        swap_guild_member_uids(
            &mut self.level,
            old_player_uid,
            new_player_uid,
            old_instance_id,
            new_instance_id,
        )?;

        progress("Swapping ownership references across all data...");
        props::swap_uuid_values_deep(
            self.level_properties_mut(),
            &OWNERSHIP_KEYS,
            old_player_uid,
            new_player_uid,
        );

        progress("Swapping player file references...");
        self.swap_player_file_refs(old_player_uid, new_player_uid);

        progress("Rebuilding caches...");
        self.rebuild_player_caches()?;

        Ok(())
    }
}
