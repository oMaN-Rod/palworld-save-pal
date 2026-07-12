//! Cross-player UID swap -- port of `game/mixins/player_swap.py`'s
//! `PlayerSwapMixin.swap_player_uids` and its private helpers
//! (`_validate_swap_players`, `_swap_player_gvas_uids`,
//! `_swap_character_save_parameters`, `_swap_guild_member_uids` +
//! `_swap_guild_character_handles`/`_swap_guild_admin_uid`/
//! `_swap_guild_player_list`, `_swap_player_file_refs`).
//!
//! Reconciliation with the brief: the brief's assumed interface
//! (`CharacterMapEntry::set_key_player_uid`, `GroupMapEntry::
//! swap_player_uids`, `SaveSession::player_file_refs()`/`player_gvas()`/
//! `character_map_mut()`/`level_properties_mut()` as zero-arg METHODS) does
//! not exist as such -- there are no typed wrapper structs over
//! `uesave::MapEntry` in this port (Task 3E-2/transfer.rs already
//! established this). This module is written directly against the same
//! real navigation layer transfer.rs uses: `domain::world`'s
//! character/group-map accessors and entry helpers (now including this
//! task's own `world::set_entry_player_uid` write helper),
//! `domain::player`'s `save_data_props`, `domain::guild_tail`'s
//! `GuildTail`/`entry_group_type`/`entry_group_data_mut` raw-tail codec,
//! and `transfer::ensure_player_gvas_loaded`/`transfer::
//! save_data_instance_id` (both promoted to `pub(crate)` by this task so
//! this module can reuse them verbatim instead of re-implementing on-demand
//! GVAS loading a second time). `SaveSession::player_file_refs` and
//! `SaveSession::loaded_players` are plain fields, not methods -- called as
//! such throughout. The `level_properties_mut`/`swap_player_file_refs`/
//! `swap_player_gvas_uids` session methods ARE real (this task adds them to
//! `session.rs`, matching the brief's exact signatures).
//!
//! The brief's own literal `SaveSession::swap_player_uids` code block (its
//! Step 3) omits `_validate_swap_players`'s level>=2 gate and its "Failed to
//! load player save files."/"Player {uid} not found in save." wording --
//! reproduced here exactly as the brief's code gives it (bare "Player {uid}
//! not found.", no level check), not as `player_swap.py` gives it. The
//! brief's progress-string list is explicitly scoped to `player_swap.py:
//! 79-120`, which also excludes line 123's trailing `"UID swap complete!"`
//! progress message -- also not reproduced here, matching that scoping.

use crate::domain::guild_tail;
use crate::domain::{player, world};
use crate::error::CoreError;
use crate::progress::ProgressSink;
use crate::props;
use crate::session::SaveSession;
use crate::transfer::{self, TransferError};
use uuid::Uuid;

/// `_OWNERSHIP_KEYS` (`player_swap.py:19-24`).
const OWNERSHIP_KEYS: [&str; 4] = [
    "OwnerPlayerUId",
    "owner_player_uid",
    "build_player_uid",
    "private_lock_player_uid",
];

/// Port of `_swap_guild_member_uids` (`player_swap.py:191-263`), folding its
/// three sub-helpers (`_swap_guild_character_handles`/`_swap_guild_admin_uid`/
/// `_swap_guild_player_list`) into one pass per Guild-type
/// `GroupSaveDataMap` entry, exactly as Python calls all three per entry:
///
/// * `individual_character_handle_ids` (a typed `PalGroupData` field, not
///   part of the raw guild tail -- see `domain::guild_tail`'s own doc
///   comment on the RawData split) -- any handle whose `instance_id`
///   matches either player's own character instance id has its `guid`
///   field retargeted to the OTHER player;
/// * the tail's `admin_player_uid`;
/// * every tail `players[].player_uid`.
///
/// Each swapped bidirectionally (old->new, new->old), matching Python's
/// `are_equal_uuids(x, old) -> new` / `are_equal_uuids(x, new) -> old`
/// pattern. Python's own guard is `"Guild" not in str(group_type): continue`
/// (a substring check); this instead matches the exact enum variant name
/// `"EPalGroupType::Guild"`, the same precision `transfer.rs`'s own guild
/// code already established for this exact check.
fn swap_guild_member_uids(
    level: &mut uesave::Save,
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

        // The admin uid and every member `player_uid` are swapped
        // bidirectionally in place; uesave re-serializes the structured guild
        // on save, and all other tail fields (roles, names, markers) survive
        // untouched.
        let Some(guild) = guild_tail::as_guild_mut(group_data) else {
            continue;
        };
        guild_tail::swap_player_uids(guild, old_uid, new_uid);
    }
    Ok(())
}

impl SaveSession {
    /// Port of `PlayerSwapMixin.swap_player_uids` (`player_swap.py:61-124`).
    /// `Ok(())` == Python's `{"success": True}`;
    /// `Err(TransferError::Rejected(msg))` == Python's `{"error": msg}` (a
    /// SOFT rejection -- the handler maps this to a normal `{"error": ...}`
    /// response on the `swap_player_uids` wire type, never the hard WS
    /// `error` frame). `TransferError` is reused verbatim from Task 3E-2's
    /// `transfer.rs`, which already carries this exact soft/hard split.
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
