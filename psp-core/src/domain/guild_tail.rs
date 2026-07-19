//! Accessors and mutation helpers over `uesave`'s structured guild group.
//!
//! A guild's binary TAIL comes in two shapes, and a real save can be either:
//! `PalGuildTail::PreUpdate` (`admin_player_uid` + `players` + trailing bytes)
//! or `PalGuildTail::PostUpdate` (which additionally carries
//! `guild_chest_allowed_roles`, a per-player `role` byte, and
//! `role_permissions`). Every accessor and mutator here handles both and never
//! assumes one.
//!
//! Mutations touch only the field being changed, leaving markers, roles, role
//! permissions, and trailing bytes intact -- `uesave` re-serializes the guild
//! byte-for-byte on save, so any field clobbered here is a field corrupted in
//! the save.

use crate::props;
use crate::ue::games::palworld::{
    PalGroupData, PalGroupVariant, PalGuildGroup, PalGuildPlayerWithRole, PalGuildTail,
    PalGuildTailPreUpdate, PalPlayerInfo, PalPlayerInfoDetails,
};
use crate::ue::{MapEntry, Property, StructValue};
use uuid::Uuid;

/// One guild member, independent of which tail shape it is written into.
pub struct GuildPlayerInfo {
    pub player_uid: Uuid,
    pub last_online_real_time: i64,
    pub player_name: String,
}

/// `entry.value.GroupType`, as its fully qualified enum variant name (e.g.
/// `"EPalGroupType::Guild"`).
pub fn entry_group_type(entry: &MapEntry) -> Option<String> {
    let value_properties = props::struct_properties(&entry.value)?;
    props::get(value_properties, &["GroupType"])
        .and_then(props::as_enum)
        .map(str::to_string)
}

/// A `GroupSaveDataMap` entry's `RawData` as structured `PalGroupData`. Does
/// not check the group type; callers that need a guild must do that themselves.
pub fn entry_group_data(entry: &MapEntry) -> Option<&PalGroupData> {
    let value_properties = props::struct_properties(&entry.value)?;
    match props::get(value_properties, &["RawData"])? {
        Property::Struct(StructValue::Game(crate::ue::PalStruct::GroupData(data))) => Some(data),
        _ => None,
    }
}

pub fn entry_group_data_mut(entry: &mut MapEntry) -> Option<&mut PalGroupData> {
    let value_properties = props::struct_props_mut(&mut entry.value)?;
    match props::get_mut(value_properties, &["RawData"])? {
        Property::Struct(StructValue::Game(crate::ue::PalStruct::GroupData(data))) => Some(data),
        _ => None,
    }
}

/// The `PalGuildGroup` inside a `PalGroupData`, if its variant is `Guild`. Any
/// other variant (`IndependentGuild`, `Organization`, `Unknown`) yields `None`
/// for the caller to skip.
pub fn as_guild(group_data: &PalGroupData) -> Option<&PalGuildGroup> {
    match &group_data.data {
        PalGroupVariant::Guild(guild) => Some(guild),
        _ => None,
    }
}

pub fn as_guild_mut(group_data: &mut PalGroupData) -> Option<&mut PalGuildGroup> {
    match &mut group_data.data {
        PalGroupVariant::Guild(guild) => Some(guild),
        _ => None,
    }
}

pub fn guild_admin_uid(guild: &PalGuildGroup) -> Uuid {
    match &guild.tail {
        PalGuildTail::PreUpdate(tail) => props::guid_to_uuid(&tail.admin_player_uid),
        PalGuildTail::PostUpdate(tail) => props::guid_to_uuid(&tail.admin_player_uid),
    }
}

/// Every member's `player_uid`, in tail order. The first is the guild admin.
pub fn guild_player_uids(guild: &PalGuildGroup) -> Vec<Uuid> {
    match &guild.tail {
        PalGuildTail::PreUpdate(tail) => tail
            .players
            .iter()
            .map(|player| props::guid_to_uuid(&player.player_uid))
            .collect(),
        PalGuildTail::PostUpdate(tail) => tail
            .players
            .iter()
            .map(|player| props::guid_to_uuid(&player.player_uid))
            .collect(),
    }
}

pub fn guild_player_count(guild: &PalGuildGroup) -> usize {
    match &guild.tail {
        PalGuildTail::PreUpdate(tail) => tail.players.len(),
        PalGuildTail::PostUpdate(tail) => tail.players.len(),
    }
}

pub fn guild_has_player(guild: &PalGuildGroup, uid: Uuid) -> bool {
    match &guild.tail {
        PalGuildTail::PreUpdate(tail) => tail
            .players
            .iter()
            .any(|player| props::guid_to_uuid(&player.player_uid) == uid),
        PalGuildTail::PostUpdate(tail) => tail
            .players
            .iter()
            .any(|player| props::guid_to_uuid(&player.player_uid) == uid),
    }
}

/// `(last_online_real_time, player_name)` for `uid`, if a member.
pub fn find_player_membership(guild: &PalGuildGroup, uid: Uuid) -> Option<(i64, String)> {
    match &guild.tail {
        PalGuildTail::PreUpdate(tail) => tail
            .players
            .iter()
            .find(|player| props::guid_to_uuid(&player.player_uid) == uid)
            .map(|player| {
                (
                    player.player_info.last_online_real_time,
                    player.player_info.player_name.clone(),
                )
            }),
        PalGuildTail::PostUpdate(tail) => tail
            .players
            .iter()
            .find(|player| props::guid_to_uuid(&player.player_uid) == uid)
            .map(|player| {
                (
                    player.player_info.last_online_real_time,
                    player.player_info.player_name.clone(),
                )
            }),
    }
}

/// The `role` byte on `uid`'s row. `None` for a `PreUpdate` guild, whose tail
/// has no per-player roles at all, or when `uid` is not a member.
pub fn player_role(guild: &PalGuildGroup, uid: Uuid) -> Option<u8> {
    match &guild.tail {
        PalGuildTail::PreUpdate(_) => None,
        PalGuildTail::PostUpdate(tail) => tail
            .players
            .iter()
            .find(|player| props::guid_to_uuid(&player.player_uid) == uid)
            .map(|player| player.role),
    }
}

/// Removes every member row matching `uid`, returning the removed row's `role`
/// (`PostUpdate` only) so a caller retargeting that slot can preserve it.
pub fn remove_player(guild: &mut PalGuildGroup, uid: Uuid) -> Option<u8> {
    match &mut guild.tail {
        PalGuildTail::PreUpdate(tail) => {
            tail.players
                .retain(|player| props::guid_to_uuid(&player.player_uid) != uid);
            None
        }
        PalGuildTail::PostUpdate(tail) => {
            let removed_role = tail
                .players
                .iter()
                .find(|player| props::guid_to_uuid(&player.player_uid) == uid)
                .map(|player| player.role);
            tail.players
                .retain(|player| props::guid_to_uuid(&player.player_uid) != uid);
            removed_role
        }
    }
}

/// Appends a member row. `role` defaults to `0` for a `PostUpdate` guild and is
/// ignored entirely for a `PreUpdate` one.
pub fn push_player(guild: &mut PalGuildGroup, member: &GuildPlayerInfo, role: Option<u8>) {
    match &mut guild.tail {
        PalGuildTail::PreUpdate(tail) => tail.players.push(PalPlayerInfo {
            player_uid: props::uuid_to_guid(member.player_uid),
            player_info: PalPlayerInfoDetails {
                last_online_real_time: member.last_online_real_time,
                player_name: member.player_name.clone(),
            },
        }),
        PalGuildTail::PostUpdate(tail) => tail.players.push(PalGuildPlayerWithRole {
            player_uid: props::uuid_to_guid(member.player_uid),
            player_info: PalPlayerInfoDetails {
                last_online_real_time: member.last_online_real_time,
                player_name: member.player_name.clone(),
            },
            role: role.unwrap_or(0),
        }),
    }
}

/// Sets `last_online_real_time` on EVERY member row matching `uid` -- a tail can
/// carry duplicate rows for one player, and all of them must stay in sync.
pub fn set_player_last_online(guild: &mut PalGuildGroup, uid: Uuid, ticks: i64) {
    match &mut guild.tail {
        PalGuildTail::PreUpdate(tail) => {
            for player in tail.players.iter_mut() {
                if props::guid_to_uuid(&player.player_uid) == uid {
                    player.player_info.last_online_real_time = ticks;
                }
            }
        }
        PalGuildTail::PostUpdate(tail) => {
            for player in tail.players.iter_mut() {
                if props::guid_to_uuid(&player.player_uid) == uid {
                    player.player_info.last_online_real_time = ticks;
                }
            }
        }
    }
}

/// Bidirectionally swaps `old_uid` <-> `new_uid` across the guild's
/// `admin_player_uid` and every member `player_uid`. Roles, names, timestamps,
/// and markers are untouched.
pub fn swap_player_uids(guild: &mut PalGuildGroup, old_uid: Uuid, new_uid: Uuid) {
    fn swapped(current: Uuid, old_uid: Uuid, new_uid: Uuid) -> Option<Uuid> {
        if current == old_uid {
            Some(new_uid)
        } else if current == new_uid {
            Some(old_uid)
        } else {
            None
        }
    }
    match &mut guild.tail {
        PalGuildTail::PreUpdate(tail) => {
            let admin = props::guid_to_uuid(&tail.admin_player_uid);
            if let Some(target) = swapped(admin, old_uid, new_uid) {
                tail.admin_player_uid = props::uuid_to_guid(target);
            }
            for player in tail.players.iter_mut() {
                let current = props::guid_to_uuid(&player.player_uid);
                if let Some(target) = swapped(current, old_uid, new_uid) {
                    player.player_uid = props::uuid_to_guid(target);
                }
            }
        }
        PalGuildTail::PostUpdate(tail) => {
            let admin = props::guid_to_uuid(&tail.admin_player_uid);
            if let Some(target) = swapped(admin, old_uid, new_uid) {
                tail.admin_player_uid = props::uuid_to_guid(target);
            }
            for player in tail.players.iter_mut() {
                let current = props::guid_to_uuid(&player.player_uid);
                if let Some(target) = swapped(current, old_uid, new_uid) {
                    player.player_uid = props::uuid_to_guid(target);
                }
            }
        }
    }
}

/// Resets a cloned template guild to a fresh single-member guild owned by
/// `admin_uid`. Preserves the tail shape and every non-member field it carries
/// (`guild_chest_allowed_roles`, `role_permissions`, trailing bytes).
pub fn reset_to_single_member(
    guild: &mut PalGuildGroup,
    guild_name: &str,
    admin_uid: Uuid,
    member: &GuildPlayerInfo,
    role: Option<u8>,
) {
    guild.guild_name = guild_name.to_string();
    guild.base_ids = Vec::new();
    guild.base_camp_level = 1;
    guild.map_object_instance_ids_base_camp_points = Vec::new();
    match &mut guild.tail {
        PalGuildTail::PreUpdate(tail) => {
            tail.admin_player_uid = props::uuid_to_guid(admin_uid);
            tail.players = vec![PalPlayerInfo {
                player_uid: props::uuid_to_guid(member.player_uid),
                player_info: PalPlayerInfoDetails {
                    last_online_real_time: member.last_online_real_time,
                    player_name: member.player_name.clone(),
                },
            }];
        }
        PalGuildTail::PostUpdate(tail) => {
            tail.admin_player_uid = props::uuid_to_guid(admin_uid);
            tail.players = vec![PalGuildPlayerWithRole {
                player_uid: props::uuid_to_guid(member.player_uid),
                player_info: PalPlayerInfoDetails {
                    last_online_real_time: member.last_online_real_time,
                    player_name: member.player_name.clone(),
                },
                role: role.unwrap_or(0),
            }];
        }
    }
}

/// Builds a `PalGuildGroup` with a `PreUpdate` tail, for tests and synthetic
/// sessions. Production never builds a guild from scratch -- transfer clones a
/// template instead.
pub fn pre_update_guild(
    base_camp_level: i32,
    guild_name: &str,
    admin_player_uid: Uuid,
    players: &[(Uuid, i64, &str)],
) -> PalGuildGroup {
    PalGuildGroup {
        org_type: 0,
        leading_bytes: [0; 4],
        base_ids: Vec::new(),
        unknown_1: 0,
        base_camp_level,
        map_object_instance_ids_base_camp_points: Vec::new(),
        guild_name: guild_name.to_string(),
        last_guild_name_modifier_player_uid: crate::ue::FGuid::nil(),
        guild_markers: Vec::new(),
        tail: PalGuildTail::PreUpdate(PalGuildTailPreUpdate {
            admin_player_uid: props::uuid_to_guid(admin_player_uid),
            players: players
                .iter()
                .map(|(uid, last_online, name)| PalPlayerInfo {
                    player_uid: props::uuid_to_guid(*uid),
                    player_info: PalPlayerInfoDetails {
                        last_online_real_time: *last_online,
                        player_name: (*name).to_string(),
                    },
                })
                .collect(),
            trailing_bytes: [0; 4],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::games::palworld::PalGuildTailPostUpdate;

    fn uid(byte: u8) -> Uuid {
        Uuid::from_bytes([byte; 16])
    }

    fn post_update_guild() -> PalGuildGroup {
        let mut guild = pre_update_guild(3, "Post", uid(1), &[]);
        guild.tail = PalGuildTail::PostUpdate(PalGuildTailPostUpdate {
            guild_chest_allowed_roles: vec![7, 8],
            unknown_i32: 42,
            admin_player_uid: props::uuid_to_guid(uid(1)),
            players: vec![
                PalGuildPlayerWithRole {
                    player_uid: props::uuid_to_guid(uid(1)),
                    player_info: PalPlayerInfoDetails {
                        last_online_real_time: 100,
                        player_name: "Admin".to_string(),
                    },
                    role: 5,
                },
                PalGuildPlayerWithRole {
                    player_uid: props::uuid_to_guid(uid(2)),
                    player_info: PalPlayerInfoDetails {
                        last_online_real_time: 200,
                        player_name: "Member".to_string(),
                    },
                    role: 2,
                },
            ],
            role_permissions: Vec::new(),
            trailing_bytes: [9; 4],
        });
        guild
    }

    #[test]
    fn accessors_read_pre_update_shape() {
        let guild = pre_update_guild(4, "Pre", uid(1), &[(uid(1), 10, "A"), (uid(2), 20, "B")]);
        assert_eq!(guild_admin_uid(&guild), uid(1));
        assert_eq!(guild_player_uids(&guild), vec![uid(1), uid(2)]);
        assert_eq!(guild_player_count(&guild), 2);
        assert!(guild_has_player(&guild, uid(2)));
        assert!(!guild_has_player(&guild, uid(9)));
        assert_eq!(
            find_player_membership(&guild, uid(2)),
            Some((20, "B".to_string()))
        );
        assert_eq!(player_role(&guild, uid(1)), None);
    }

    #[test]
    fn accessors_read_post_update_shape() {
        let guild = post_update_guild();
        assert_eq!(guild_admin_uid(&guild), uid(1));
        assert_eq!(guild_player_uids(&guild), vec![uid(1), uid(2)]);
        assert_eq!(guild_player_count(&guild), 2);
        assert_eq!(
            find_player_membership(&guild, uid(2)),
            Some((200, "Member".to_string()))
        );
        assert_eq!(player_role(&guild, uid(2)), Some(2));
    }

    #[test]
    fn remove_player_returns_role_and_preserves_other_rows_post_update() {
        let mut guild = post_update_guild();
        assert_eq!(remove_player(&mut guild, uid(2)), Some(2));
        assert_eq!(guild_player_uids(&guild), vec![uid(1)]);
        // Non-member fields survive untouched.
        if let PalGuildTail::PostUpdate(tail) = &guild.tail {
            assert_eq!(tail.guild_chest_allowed_roles, vec![7, 8]);
            assert_eq!(tail.unknown_i32, 42);
            assert_eq!(tail.trailing_bytes, [9; 4]);
        } else {
            panic!("shape must stay PostUpdate");
        }
    }

    #[test]
    fn push_player_uses_role_for_post_update_only() {
        let member = GuildPlayerInfo {
            player_uid: uid(3),
            last_online_real_time: 300,
            player_name: "New".to_string(),
        };

        let mut pre = pre_update_guild(1, "Pre", uid(1), &[]);
        push_player(&mut pre, &member, Some(9));
        assert_eq!(guild_player_uids(&pre), vec![uid(3)]);
        assert_eq!(player_role(&pre, uid(3)), None);

        let mut post = post_update_guild();
        push_player(&mut post, &member, Some(9));
        assert_eq!(player_role(&post, uid(3)), Some(9));
    }

    #[test]
    fn swap_player_uids_swaps_admin_and_members_both_shapes() {
        for mut guild in [
            pre_update_guild(1, "G", uid(1), &[(uid(1), 0, "A"), (uid(2), 0, "B")]),
            post_update_guild(),
        ] {
            swap_player_uids(&mut guild, uid(1), uid(2));
            assert_eq!(guild_admin_uid(&guild), uid(2));
            assert_eq!(guild_player_uids(&guild), vec![uid(2), uid(1)]);
        }
    }

    #[test]
    fn reset_to_single_member_keeps_post_update_shape_and_perms() {
        let mut guild = post_update_guild();
        if let PalGuildTail::PostUpdate(tail) = &mut guild.tail {
            tail.role_permissions.clear();
            tail.guild_chest_allowed_roles = vec![1, 2, 3];
        }
        let member = GuildPlayerInfo {
            player_uid: uid(7),
            last_online_real_time: 700,
            player_name: "Solo".to_string(),
        };
        reset_to_single_member(&mut guild, "Transferred Guild", uid(7), &member, Some(4));
        assert_eq!(guild.guild_name, "Transferred Guild");
        assert_eq!(guild.base_camp_level, 1);
        assert!(guild.base_ids.is_empty());
        assert_eq!(guild_admin_uid(&guild), uid(7));
        assert_eq!(guild_player_uids(&guild), vec![uid(7)]);
        assert_eq!(player_role(&guild, uid(7)), Some(4));
        if let PalGuildTail::PostUpdate(tail) = &guild.tail {
            assert_eq!(tail.guild_chest_allowed_roles, vec![1, 2, 3]);
        } else {
            panic!("shape must stay PostUpdate");
        }
    }
}
