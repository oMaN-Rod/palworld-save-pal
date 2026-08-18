mod common;

use psp_core::domain::blueprint::place::PlacementRequest;
use psp_core::domain::blueprint::validate::{Anchor, Finding, PlacementMode, Severity};
use psp_core::domain::blueprint::{capture, place, validate, BaseBlueprint, CaptureOptions};
use psp_core::gamedata::GameData;
use psp_core::session::SaveSession;
use std::collections::BTreeSet;
use uuid::Uuid;

/// The one `GroupSaveDataMap` group type that owns bases; `place` refuses any
/// other outright.
const GUILD_GROUP_TYPE: &str = "EPalGroupType::Guild";

fn game_data() -> GameData {
    common::game_data()
}

fn anchor_far_from_everything() -> Anchor {
    Anchor {
        x: 400_000.0,
        y: 400_000.0,
        z: 1000.0,
        yaw_radians: 0.0,
    }
}

/// A warnings-overridden request, the shape every placement test but
/// `warnings_are_refused_unless_overridden` wants.
fn request(anchor: Anchor, mode: PlacementMode, owner: Uuid) -> PlacementRequest {
    PlacementRequest {
        anchor,
        mode,
        owner_player_uid: owner,
        override_warnings: true,
    }
}

fn new_base_request(anchor: Anchor, guild_id: Uuid, owner: Uuid) -> PlacementRequest {
    request(anchor, PlacementMode::NewBase { guild_id }, owner)
}

fn merge_request(anchor: Anchor, base_id: Uuid, owner: Uuid) -> PlacementRequest {
    request(anchor, PlacementMode::MergeInto { base_id }, owner)
}

/// The corpus fixture ships no `WorldOption.sav`, and every limit check reads
/// its answer from one -- without this, all three Blocking checks return before
/// they compare anything and any "nothing is blocked" assertion is vacuous.
fn session_with_limits() -> SaveSession {
    let mut session = common::load_fixture_session("v1_relics");
    common::attach_world_option(&mut session, "save_files");
    session
}

fn blueprint_of(session: &SaveSession, base_id: Uuid) -> BaseBlueprint {
    capture::capture(session, base_id, CaptureOptions::blueprint(), "Home").expect("capture")
}

fn with_code<'a>(findings: &'a [Finding], code: &str) -> Vec<&'a Finding> {
    findings
        .iter()
        .filter(|finding| finding.code == code)
        .collect()
}

fn count_code(findings: &[Finding], code: &str) -> usize {
    with_code(findings, code).len()
}

/// A structure's horizontal distance from the blueprint's anchor, the quantity
/// `outside_area_range` compares against the target's `area_range`.
fn horizontal_offset(structure: &psp_core::domain::blueprint::BlueprintStructure) -> f64 {
    let translation = &structure.relative_transform.translation;
    (translation.x.0 * translation.x.0 + translation.y.0 * translation.y.0).sqrt()
}

#[test]
fn a_clean_placement_produces_no_blocking_findings() {
    let session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);

    // The limits have to be readable AND leave room, or "not blocked" is true
    // by construction rather than by decision.
    assert_eq!(
        common::world_option_int(&session, "BaseCampMaxNum"),
        Some(128)
    );
    assert_eq!(
        common::world_option_int(&session, "BaseCampMaxNumInGuild"),
        Some(10)
    );
    assert_eq!(
        common::world_option_int(&session, "MaxBuildingLimitNum"),
        Some(0)
    );
    let guilds = common::base_camp_guild_ids(&session);
    let guild_bases = guilds.iter().filter(|owner| **owner == guild_id).count();
    assert!(
        guilds.len() < 128,
        "world has {} bases, not under the limit",
        guilds.len()
    );
    assert!(
        guild_bases < 10,
        "guild has {guild_bases} bases, not under the limit"
    );

    let findings = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );

    assert!(
        !validate::has_blocking(&findings),
        "a far-away placement into a guild with room must not be blocked: {findings:?}"
    );
    assert!(
        findings.is_empty(),
        "a far-away placement into a guild with room must produce no findings at all: {findings:?}"
    );
}

#[test]
fn placing_on_top_of_the_source_base_warns() {
    let session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let source_anchor = common::fixture_base_anchor(&session, base_id);

    let findings = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &source_anchor,
        &PlacementMode::NewBase { guild_id },
    );

    assert!(
        findings
            .iter()
            .any(|f| f.severity == Severity::Warning && f.code == "base_too_close"),
        "placing a base on top of an existing one must warn: {findings:?}"
    );
    // The binding rule: proximity is advice, never a refusal.
    assert!(
        !validate::has_blocking(&findings),
        "base_too_close must never make a placement blocking: {findings:?}"
    );
}

#[test]
fn a_guild_at_its_base_limit_blocks_placement() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    common::set_world_option_int(&mut session, "BaseCampMaxNumInGuild", 1);

    let findings = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );

    assert!(
        findings
            .iter()
            .any(|f| f.severity == Severity::Blocking && f.code == "guild_base_limit"),
        "a full guild must block a new base: {findings:?}"
    );
}

/// `BaseCampMaxNumInGuild` is per guild, not per world. The fixture's bases are
/// spread over nine guilds, so a limit above the target guild's own count but
/// far below the world's total separates the two readings.
#[test]
fn the_guild_base_limit_counts_only_the_target_guilds_bases() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);

    let guilds = common::base_camp_guild_ids(&session);
    let world_bases = guilds.len();
    let guild_bases = guilds.iter().filter(|owner| **owner == guild_id).count();
    assert!(
        guild_bases < world_bases,
        "fixture must own fewer bases than the world holds: {guild_bases} of {world_bases}"
    );

    common::set_world_option_int(&mut session, "BaseCampMaxNumInGuild", guild_bases as i32);
    let at_limit = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );
    assert_eq!(
        count_code(&at_limit, "guild_base_limit"),
        1,
        "a guild holding {guild_bases} bases is at a limit of {guild_bases}: {at_limit:?}"
    );

    common::set_world_option_int(
        &mut session,
        "BaseCampMaxNumInGuild",
        guild_bases as i32 + 1,
    );
    let under_limit = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );
    assert_eq!(
        count_code(&under_limit, "guild_base_limit"),
        0,
        "a guild holding {guild_bases} bases is under a limit of {}; counting all {world_bases} \
         world bases instead would have blocked it: {under_limit:?}",
        guild_bases + 1
    );
}

/// `BaseCampMaxNum` is the world-wide cap, evaluated against every base camp in
/// the save rather than the target guild's share of them.
#[test]
fn the_world_base_limit_blocks_at_the_world_base_count() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let world_bases = common::base_camp_guild_ids(&session).len();

    common::set_world_option_int(&mut session, "BaseCampMaxNum", world_bases as i32);
    let at_limit = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );
    let blocked = with_code(&at_limit, "world_base_limit");
    assert_eq!(
        blocked.len(),
        1,
        "a world holding {world_bases} bases is at a limit of {world_bases}: {at_limit:?}"
    );
    assert_eq!(blocked[0].severity, Severity::Blocking);

    common::set_world_option_int(&mut session, "BaseCampMaxNum", world_bases as i32 + 1);
    let under_limit = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );
    assert_eq!(
        count_code(&under_limit, "world_base_limit"),
        0,
        "a world holding {world_bases} bases is under a limit of {}: {under_limit:?}",
        world_bases + 1
    );
}

/// `MaxBuildingLimitNum` caps the structure count the placement would leave
/// behind; 0 is the game's "no limit" value and must never fire.
#[test]
fn the_building_limit_blocks_only_when_the_placement_would_exceed_it() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let structure_count = blueprint.structures.len();
    assert!(
        structure_count > 1,
        "fixture base must carry real structures"
    );

    let check = |session: &SaveSession| {
        validate::check(
            session,
            &game_data(),
            &blueprint,
            &anchor_far_from_everything(),
            &PlacementMode::NewBase { guild_id },
        )
    };

    common::set_world_option_int(
        &mut session,
        "MaxBuildingLimitNum",
        structure_count as i32 - 1,
    );
    let over = check(&session);
    let blocked = with_code(&over, "building_limit");
    assert_eq!(
        blocked.len(),
        1,
        "{structure_count} structures exceed a limit of {}: {over:?}",
        structure_count - 1
    );
    assert_eq!(blocked[0].severity, Severity::Blocking);

    common::set_world_option_int(&mut session, "MaxBuildingLimitNum", structure_count as i32);
    assert_eq!(
        count_code(&check(&session), "building_limit"),
        0,
        "{structure_count} structures exactly fill a limit of {structure_count}"
    );

    common::set_world_option_int(&mut session, "MaxBuildingLimitNum", 0);
    assert_eq!(
        count_code(&check(&session), "building_limit"),
        0,
        "MaxBuildingLimitNum 0 means unlimited"
    );
}

/// A blueprint's own structures are bounded by the radius they were captured
/// against, so `outside_area_range` can only fire when the blueprint lands in a
/// base whose footprint is tighter than the one it came from.
#[test]
fn outside_area_range_fires_when_merging_into_a_smaller_footprint() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let anchor = common::fixture_base_anchor(&session, base_id);

    let unchanged = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor,
        &PlacementMode::MergeInto { base_id },
    );
    assert_eq!(
        count_code(&unchanged, "outside_area_range"),
        0,
        "merging a base back into itself fits its own footprint: {unchanged:?}"
    );

    let tighter_radius = 2000.0_f64;
    let expected = blueprint
        .structures
        .iter()
        .filter(|structure| horizontal_offset(structure) > tighter_radius)
        .count();
    assert!(
        expected > 0 && expected < blueprint.structures.len(),
        "the fixture base must straddle a {tighter_radius} cm radius, got {expected} of {}",
        blueprint.structures.len()
    );
    common::set_base_area_range(&mut session, base_id, tighter_radius as f32);

    let findings = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor,
        &PlacementMode::MergeInto { base_id },
    );

    assert_eq!(
        count_code(&findings, "outside_area_range"),
        expected,
        "every structure beyond the target base's {tighter_radius} cm radius must be reported"
    );
    assert!(
        !validate::has_blocking(&findings),
        "outside_area_range is a warning: {findings:?}"
    );
}

/// Dropping a blueprint back onto the ground its own base occupies collides
/// with every one of that base's placed structures, because a new base owns
/// none of them.
#[test]
fn structure_overlap_reports_every_colliding_structure() {
    let session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let anchor = common::fixture_base_anchor(&session, base_id);

    let on_top = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor,
        &PlacementMode::NewBase { guild_id },
    );
    assert_eq!(
        count_code(&on_top, "structure_overlap"),
        blueprint.structures.len(),
        "each structure lands exactly on the object it was captured from: {on_top:?}"
    );

    let far_away = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );
    assert_eq!(
        count_code(&far_away, "structure_overlap"),
        0,
        "an empty stretch of map collides with nothing: {far_away:?}"
    );
}

/// `MergeInto` adds to a base rather than founding one: the target's own
/// structures are not foreign objects to collide with, but they do count
/// against the building limit.
#[test]
fn merge_into_skips_the_targets_own_structures_and_sums_its_building_count() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let anchor = common::fixture_base_anchor(&session, base_id);
    let structure_count = blueprint.structures.len();

    let merged = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor,
        &PlacementMode::MergeInto { base_id },
    );
    assert_eq!(
        count_code(&merged, "structure_overlap"),
        0,
        "the target base's own structures are not another base's: {merged:?}"
    );
    assert_eq!(
        count_code(&merged, "base_too_close"),
        0,
        "merging founds no base, so base separation does not apply: {merged:?}"
    );
    assert_eq!(
        count_code(&merged, "guild_base_limit") + count_code(&merged, "world_base_limit"),
        0,
        "merging founds no base, so base counts do not apply: {merged:?}"
    );

    // The target already holds `structure_count` structures and the blueprint
    // brings the same number again, so a limit one under the sum must block a
    // merge while leaving a brand new base of the same blueprint alone.
    common::set_world_option_int(
        &mut session,
        "MaxBuildingLimitNum",
        2 * structure_count as i32 - 1,
    );
    let merged = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor,
        &PlacementMode::MergeInto { base_id },
    );
    assert_eq!(
        count_code(&merged, "building_limit"),
        1,
        "a merge must count the target's existing {structure_count} structures too: {merged:?}"
    );
    let new_base = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );
    assert_eq!(
        count_code(&new_base, "building_limit"),
        0,
        "a new base only brings the blueprint's own {structure_count} structures: {new_base:?}"
    );
}

/// A save whose `WorldOption.sav` is missing or unparseable loads fine -- the
/// loader degrades to `None` and warns. Validation must say the limits went
/// unchecked instead of reporting a clean bill of health.
#[test]
fn limits_that_cannot_be_read_are_reported_rather_than_passed() {
    let session = common::load_fixture_session("v1_relics");
    assert!(
        session.world_option.is_none(),
        "the corpus fixture ships no WorldOption.sav"
    );
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);

    let findings = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );

    let unknown = with_code(&findings, "limits_unknown");
    assert_eq!(
        unknown.len(),
        1,
        "an unreadable limit set must be reported once: {findings:?}"
    );
    assert_eq!(unknown[0].severity, Severity::Warning);
    for key in [
        "BaseCampMaxNumInGuild",
        "BaseCampMaxNum",
        "MaxBuildingLimitNum",
    ] {
        assert!(
            unknown[0].message.contains(key),
            "the finding must name {key}: {}",
            unknown[0].message
        );
    }
    for code in ["guild_base_limit", "world_base_limit", "building_limit"] {
        assert_eq!(
            count_code(&findings, code),
            0,
            "{code} cannot be decided without limits: {findings:?}"
        );
    }
}

/// The bundled buildings catalog is what tells placement a structure is a real
/// build object. It spells ids in the game's own casing (`Stone_foundation`),
/// while saves carry the level's (`Stone_Foundation`), so the lookup has to be
/// case-insensitive or hundreds of ordinary walls read as unknown.
#[test]
fn unknown_structure_types_are_flagged_and_known_ones_are_not() {
    let session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let mut blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let data = game_data();

    let known = validate::check(
        &session,
        &data,
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );
    assert_eq!(
        count_code(&known, "unknown_structure_type"),
        0,
        "every structure of a real captured base is a catalogued building: {known:?}"
    );

    blueprint.structures[0].map_object_id = "NotARealBuildingType".to_string();
    let findings = validate::check(
        &session,
        &data,
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );

    let unknown = with_code(&findings, "unknown_structure_type");
    assert_eq!(
        unknown.len(),
        1,
        "the bogus structure must be flagged once: {findings:?}"
    );
    assert_eq!(unknown[0].severity, Severity::Warning);
    assert!(
        unknown[0].message.contains("NotARealBuildingType"),
        "the finding must name the offending id: {}",
        unknown[0].message
    );
    assert!(
        !validate::has_blocking(&findings),
        "an unrecognized id is advisory, not a refusal: {findings:?}"
    );
}

// ---- place ----

/// Everything a half-applied placement would disturb: one count for every
/// collection `commit` appends to, plus the exact set of placed structure
/// instance ids and the guild registration lists. A refusal has to leave all of
/// them untouched.
#[derive(Debug, PartialEq)]
struct SessionFingerprint {
    map_objects: usize,
    base_camps: usize,
    works: usize,
    item_containers: usize,
    character_containers: usize,
    characters: usize,
    dynamic_items: usize,
    guild_registrations: Vec<(usize, usize, usize)>,
    structure_instance_ids: BTreeSet<Uuid>,
}

/// Absent collections count as zero rather than panicking: a fixture the
/// placement is expected to REFUSE (`world2` carries no `WorkSaveData` and no
/// `BaseCampSaveData` at all) still has to be fingerprinted on both sides of
/// the refusal.
fn session_fingerprint(session: &SaveSession) -> SessionFingerprint {
    use psp_core::domain::world;

    let guild_registrations = world::group_map(&session.level)
        .map(|entries| entries.iter().map(guild_registration_counts).collect())
        .unwrap_or_default();

    SessionFingerprint {
        map_objects: common::map_object_count(session),
        base_camps: common::base_count(session),
        works: world::work_values(&session.level)
            .ok()
            .flatten()
            .map_or(0, Vec::len),
        item_containers: world::item_container_map(&session.level).map_or(0, Vec::len),
        character_containers: world::character_container_map(&session.level).map_or(0, Vec::len),
        characters: world::character_map(&session.level).map_or(0, Vec::len),
        dynamic_items: world::dynamic_item_values(&session.level).map_or(0, Vec::len),
        guild_registrations,
        structure_instance_ids: common::all_map_object_instance_ids(session)
            .into_iter()
            .collect(),
    }
}

/// `(individual_character_handle_ids, base_ids, map_object_instance_ids_base_camp_points)`
/// lengths for one `GroupSaveDataMap` entry -- the three lists a placement
/// registers itself in.
fn guild_registration_counts(entry: &psp_core::ue::MapEntry) -> (usize, usize, usize) {
    use psp_core::domain::guild_tail;

    let Some(group_data) = guild_tail::entry_group_data(entry) else {
        return (0, 0, 0);
    };
    let handles = group_data.individual_character_handle_ids.len();
    match guild_tail::as_guild(group_data) {
        Some(guild) => (
            handles,
            guild.base_ids.len(),
            guild.map_object_instance_ids_base_camp_points.len(),
        ),
        None => (handles, 0, 0),
    }
}

/// Every `GroupSaveDataMap` key whose entry is not a guild. `v1_relics` carries
/// `EPalGroupType::Organization` groups alongside its guilds, and nothing but
/// the group type distinguishes them at the map level -- which is exactly why a
/// placement can be pointed at one by accident.
fn non_guild_group_ids(session: &SaveSession) -> Vec<Uuid> {
    use psp_core::domain::{guild_tail, world};

    let entries = world::group_map(&session.level).expect("group map");
    let guilds = entries
        .iter()
        .filter(|entry| {
            guild_tail::entry_group_type(entry).as_deref() == Some("EPalGroupType::Guild")
        })
        .count();
    assert!(
        guilds > 0 && guilds < entries.len(),
        "the fixture must mix guilds with other group types, got {guilds} of {}",
        entries.len()
    );
    entries
        .iter()
        .filter(|entry| {
            guild_tail::entry_group_type(entry).as_deref() != Some("EPalGroupType::Guild")
        })
        .filter_map(|entry| psp_core::props::as_uuid(&entry.key))
        .collect()
}

/// `(GroupType, RawData decodes as group data, that group data is a guild)` --
/// the three facts `target_guild`'s two gates read, so a test can show which of
/// them an input actually trips.
fn group_gate_facts(session: &SaveSession, group_id: Uuid) -> (Option<String>, bool, bool) {
    use psp_core::domain::{guild_tail, world};

    let entry = world::group_map(&session.level)
        .expect("group map")
        .iter()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(group_id))
        .expect("the named group has a GroupSaveDataMap entry");
    let group_data = guild_tail::entry_group_data(entry);
    (
        guild_tail::entry_group_type(entry),
        group_data.is_some(),
        group_data.and_then(guild_tail::as_guild).is_some(),
    )
}

/// Relabels a group as `EPalGroupType::Guild` without touching its `RawData`,
/// producing what a Guild-typed group with a corrupt tail looks like from
/// `place`'s side: the type gate waves it through and only `as_guild` is left
/// to refuse it.
fn relabel_group_as_guild(session: &mut SaveSession, group_id: Uuid) {
    use psp_core::domain::world;

    let entry = world::group_map_mut(&mut session.level)
        .expect("group map")
        .iter_mut()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(group_id))
        .expect("the named group has a GroupSaveDataMap entry");
    psp_core::props::struct_props_mut(&mut entry.value)
        .expect("group value")
        .insert(
            "GroupType",
            psp_core::props::enum_property("EPalGroupType::Guild"),
        );
}

/// One placed structure's identity fields, read straight off
/// `MapObjectSaveData` rather than through a DTO, so ownership rebinding is
/// visible.
struct PlacedStructure {
    map_object_id: String,
    instance_id: Uuid,
    base_camp_id: Uuid,
    group_id: Uuid,
    build_player_uid: Uuid,
    /// The `base_camp_id` a `BaseCampPoint` concrete model names -- the Pal
    /// Box's own record of which base it founds, separate from the `Model`'s.
    base_camp_point_id: Option<Uuid>,
    x: f64,
    y: f64,
    z: f64,
}

fn placed_structures(session: &SaveSession, base_id: Uuid) -> Vec<PlacedStructure> {
    use psp_core::ue::games::palworld::PalMapConcreteModelVariant;
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let values = psp_core::domain::world::map_object_values(&session.level)
        .expect("map object values")
        .expect("the fixture must have MapObjectSaveData");
    values
        .iter()
        .filter_map(|value| {
            let StructValue::Struct(object_props) = value else {
                return None;
            };
            let model = object_props
                .0
                .get(&PropertyKey::from("Model"))
                .and_then(psp_core::props::struct_props)?;
            let Some(Property::Struct(StructValue::Game(PalStruct::MapModel(raw)))) =
                model.0.get(&PropertyKey::from("RawData"))
            else {
                return None;
            };
            if psp_core::props::guid_to_uuid(&raw.base_camp_id_belong_to) != base_id {
                return None;
            }
            let base_camp_point_id = object_props
                .0
                .get(&PropertyKey::from("ConcreteModel"))
                .and_then(psp_core::props::struct_props)
                .and_then(|concrete| concrete.0.get(&PropertyKey::from("RawData")))
                .and_then(|raw_data| match raw_data {
                    Property::Struct(StructValue::Game(PalStruct::MapConcreteModel(concrete))) => {
                        match &concrete.model_data {
                            PalMapConcreteModelVariant::BaseCampPoint(point) => {
                                Some(psp_core::props::guid_to_uuid(&point.base_camp_id))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                });
            let translation = &raw.initial_transform_cache.translation;
            Some(PlacedStructure {
                map_object_id: object_props
                    .0
                    .get(&PropertyKey::from("MapObjectId"))
                    .and_then(psp_core::props::as_str)
                    .unwrap_or_default()
                    .to_string(),
                instance_id: psp_core::props::guid_to_uuid(&raw.instance_id),
                base_camp_id: psp_core::props::guid_to_uuid(&raw.base_camp_id_belong_to),
                group_id: psp_core::props::guid_to_uuid(&raw.group_id_belong_to),
                build_player_uid: psp_core::props::guid_to_uuid(&raw.build_player_uid),
                base_camp_point_id,
                x: translation.x.0,
                y: translation.y.0,
                z: translation.z.0,
            })
        })
        .collect()
}

/// Positions rounded to a millimetre, so a set comparison is not at the mercy
/// of the last bit of an f64.
fn position_key(x: f64, y: f64, z: f64) -> String {
    format!("{x:.3}/{y:.3}/{z:.3}")
}

#[test]
fn placing_a_blueprint_adds_a_base_and_its_structures() {
    let mut session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);
    let bases_before = common::base_count(&session);
    let objects_before = common::map_object_count(&session);
    assert!(
        !blueprint.structures.is_empty(),
        "the blueprint must carry structures or every count below is vacuous"
    );

    let result = place::place(
        &mut session,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &game_data(),
    )
    .expect("placement must succeed");

    assert_eq!(
        common::base_count(&session),
        bases_before + 1,
        "placement must add one base"
    );
    assert_eq!(
        result.structures_placed as usize,
        blueprint.structures.len(),
        "every structure must be placed"
    );
    assert_eq!(
        common::map_object_count(&session),
        objects_before + blueprint.structures.len(),
        "every placed structure must reach MapObjectSaveData"
    );
    let placed_base = result.base_id.expect("a new base must report its id");
    assert_ne!(placed_base, base_id, "the placed base gets a fresh id");
    assert_eq!(
        placed_structures(&session, placed_base).len(),
        blueprint.structures.len(),
        "every placed structure must belong to the new base"
    );
}

#[test]
fn placed_structures_are_rebound_to_the_owner_and_the_target_guild() {
    let mut session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);
    assert!(!owner.is_nil(), "the fixture owner uid must be real");
    // Capture scrubs the builder uid to nil, so a placement that forgot to
    // rebind it would leave nil behind rather than the source save's value.
    assert!(
        capture::structure_build_player_uids(&blueprint)
            .iter()
            .all(Uuid::is_nil),
        "a captured blueprint carries no builder uid"
    );

    let result = place::place(
        &mut session,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &game_data(),
    )
    .expect("placement");

    let placed_base = result.base_id.expect("a new base must report its id");
    let structures = placed_structures(&session, placed_base);
    assert_eq!(structures.len(), blueprint.structures.len());
    for structure in &structures {
        assert_eq!(structure.base_camp_id, placed_base);
        assert_eq!(structure.group_id, guild_id);
        assert_eq!(structure.build_player_uid, owner);
    }
}

#[test]
fn a_blocked_placement_leaves_the_session_untouched() {
    let mut session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);
    common::set_world_option_int(&mut session, "BaseCampMaxNumInGuild", 1);
    let before = session_fingerprint(&session);
    assert!(
        before.map_objects > 0
            && before.base_camps > 0
            && !before.structure_instance_ids.is_empty(),
        "the fixture must carry map objects and base camps"
    );

    let result = place::place(
        &mut session,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &game_data(),
    );

    assert!(
        result.is_err(),
        "a blocking finding must fail the placement"
    );
    assert_eq!(
        session_fingerprint(&session),
        before,
        "a blocked placement must not half-apply"
    );
}

/// Every refusal that survives validation, one per stage of the run, so the
/// untouched-session guarantee is pinned all the way up to `commit` rather than
/// only at the first gate:
///
/// * an unknown merge target fails in guild resolution, before `preflight`;
/// * a group that is not a guild fails the same gate, one check later;
/// * a Guild-typed group whose data is not a guild fails the check after that,
///   which no fixture input reaches on its own: every non-Guild group in
///   `v1_relics` is refused by the type check first, so the `as_guild` gate is
///   given an input built for it;
/// * a blueprint with no base camp fails inside `preflight`;
/// * a blueprint whose base camp lost its typed `RawData` reaches `preflight`
///   intact and fails during staging, after the clone has been remapped and
///   transformed;
/// * a blueprint carrying one property name at two different types fails inside
///   `commit` itself, which is the last place a refusal can still be free.
///
/// Each case also pins the reason it was refused, because two gates that both
/// happen to reject the same input prove nothing about either one.
///
/// None of the five is a blocking finding, which is what makes them refusals
/// that happen after `validate::check` has already passed.
#[test]
fn a_placement_that_fails_after_validation_leaves_the_session_untouched() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);

    // Two distinct non-guild groups: one stays as it is to trip the GroupType
    // gate, the other is relabelled a Guild so that gate passes and the
    // `as_guild` gate is the only thing left between it and
    // `register_with_guild` -- which would hand it pal handles it can never
    // record base ids beside.
    let non_guilds = non_guild_group_ids(&session);
    assert!(
        non_guilds.len() >= 2,
        "the fixture must carry two non-guild groups, or the two gates cannot be given \
         separate inputs: {}",
        non_guilds.len()
    );
    let (not_a_guild, guild_typed_non_guild) = (non_guilds[0], non_guilds[1]);
    relabel_group_as_guild(&mut session, guild_typed_non_guild);
    assert_eq!(
        group_gate_facts(&session, guild_typed_non_guild),
        (Some(GUILD_GROUP_TYPE.to_string()), true, false),
        "the relabelled group must be Guild-typed with data that decodes but is not a guild, \
         or it pins some other gate"
    );
    assert_ne!(
        group_gate_facts(&session, not_a_guild).0.as_deref(),
        Some(GUILD_GROUP_TYPE),
        "the other group must still fail the GroupType gate"
    );

    let mut no_base_camp = blueprint.clone();
    no_base_camp.base_camp = None;
    let mut untyped_base_camp = blueprint.clone();
    untyped_base_camp
        .base_camp
        .as_mut()
        .expect("the capture carries a base camp")
        .insert("RawData", psp_core::props::int_property(0));
    // One property name, two property types, on two structures that share a
    // schema path: no single write tag can describe both.
    let mut clashing_types = blueprint.clone();
    assert!(
        clashing_types.structures.len() > 1,
        "the capture must carry several structures"
    );
    clashing_types.structures[0]
        .properties
        .insert("PspTypeClash", psp_core::props::int_property(0));
    clashing_types.structures[1].properties.insert(
        "PspTypeClash",
        psp_core::ue::Property::Str("clash".to_string()),
    );

    let cases: Vec<(&str, &BaseBlueprint, PlacementRequest, &str)> = vec![
        (
            "merge into a base that does not exist",
            &blueprint,
            merge_request(anchor_far_from_everything(), Uuid::new_v4(), owner),
            "not found",
        ),
        (
            "a group that is not a guild",
            &blueprint,
            new_base_request(anchor_far_from_everything(), not_a_guild, owner),
            "not a guild",
        ),
        (
            "a Guild-typed group whose data is not a guild",
            &blueprint,
            new_base_request(anchor_far_from_everything(), guild_typed_non_guild, owner),
            "carries no decodable guild data",
        ),
        (
            "a blueprint with no base camp",
            &no_base_camp,
            new_base_request(anchor_far_from_everything(), guild_id, owner),
            "no base camp to found a base with",
        ),
        (
            "a base camp with no typed RawData",
            &untyped_base_camp,
            new_base_request(anchor_far_from_everything(), guild_id, owner),
            "carries no typed RawData",
        ),
        (
            "one property name at two types",
            &clashing_types,
            new_base_request(anchor_far_from_everything(), guild_id, owner),
            "irreconcilable",
        ),
    ];

    for (label, candidate, placement, expected) in cases {
        let findings = validate::check(
            &session,
            &game_data(),
            candidate,
            &placement.anchor,
            &placement.mode,
        );
        assert!(
            !validate::has_blocking(&findings),
            "{label}: must not be caught by validation, or it proves nothing about staging: {findings:?}"
        );

        let before = session_fingerprint(&session);
        let error = place::place(&mut session, candidate, &placement, &game_data())
            .err()
            .unwrap_or_else(|| panic!("{label}: must fail the placement"));

        assert!(
            error.to_string().contains(expected),
            "{label}: must be refused for its own reason, expected {expected:?}, got: {error}"
        );
        assert_eq!(
            session_fingerprint(&session),
            before,
            "{label}: a placement that fails after validation must not half-apply"
        );
    }
}

#[test]
fn warnings_are_refused_unless_overridden() {
    let mut session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);
    // No `WorldOption.sav`, so `limits_unknown` fires: a warning, never blocking.
    let findings = validate::check(
        &session,
        &game_data(),
        &blueprint,
        &anchor_far_from_everything(),
        &PlacementMode::NewBase { guild_id },
    );
    assert_eq!(count_code(&findings, "limits_unknown"), 1, "{findings:?}");
    assert!(!validate::has_blocking(&findings), "{findings:?}");
    let before = session_fingerprint(&session);

    let refused = place::place(
        &mut session,
        &blueprint,
        &PlacementRequest {
            override_warnings: false,
            ..new_base_request(anchor_far_from_everything(), guild_id, owner)
        },
        &game_data(),
    );
    assert!(
        refused.is_err(),
        "a warning must stop a placement that did not override"
    );
    assert_eq!(
        session_fingerprint(&session),
        before,
        "a refusal must change nothing"
    );

    let accepted = place::place(
        &mut session,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &game_data(),
    )
    .expect("the same placement must succeed once the warnings are overridden");
    assert_eq!(
        count_code(&accepted.findings, "limits_unknown"),
        1,
        "the result must carry the findings it was allowed past: {:?}",
        accepted.findings
    );
}

#[test]
fn the_same_blueprint_can_be_placed_twice() {
    let mut session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);
    common::set_world_option_int(&mut session, "BaseCampMaxNumInGuild", 99);
    let ids_before = common::all_map_object_instance_ids(&session).len();

    let first = anchor_far_from_everything();
    let mut second = anchor_far_from_everything();
    second.x += 50_000.0;

    place::place(
        &mut session,
        &blueprint,
        &new_base_request(first, guild_id, owner),
        &game_data(),
    )
    .expect("first placement");
    place::place(
        &mut session,
        &blueprint,
        &new_base_request(second, guild_id, owner),
        &game_data(),
    )
    .expect("second placement must not collide with the first");

    let ids = common::all_map_object_instance_ids(&session);
    assert_eq!(
        ids.len(),
        ids_before + 2 * blueprint.structures.len(),
        "both placements must have landed"
    );
    let unique: BTreeSet<_> = ids.iter().collect();
    assert_eq!(
        ids.len(),
        unique.len(),
        "placing twice must not produce duplicate instance ids"
    );
}

#[test]
fn placed_structures_land_at_the_chosen_anchor() {
    let mut session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);
    let anchor = anchor_far_from_everything();
    assert_eq!(
        anchor.yaw_radians, 0.0,
        "the expectation below assumes no rotation"
    );

    // Derived from the blueprint, not from the placement: with an unrotated
    // anchor a structure's world position is exactly anchor + its captured
    // offset.
    let expected: BTreeSet<String> = blueprint
        .structures
        .iter()
        .map(|structure| {
            let offset = &structure.relative_transform.translation;
            position_key(
                anchor.x + offset.x.0,
                anchor.y + offset.y.0,
                anchor.z + offset.z.0,
            )
        })
        .collect();
    assert!(
        blueprint
            .structures
            .iter()
            .any(|s| horizontal_offset(s) > 100.0),
        "the blueprint must spread out, or the anchor and the source would be indistinguishable"
    );

    let result = place::place(
        &mut session,
        &blueprint,
        &new_base_request(anchor, guild_id, owner),
        &game_data(),
    )
    .expect("placement");

    let placed_base = result.base_id.expect("a new base must report its id");
    let structures = placed_structures(&session, placed_base);
    assert!(
        !structures.is_empty(),
        "the placed base must have structures"
    );
    let actual: BTreeSet<String> = structures
        .iter()
        .map(|structure| position_key(structure.x, structure.y, structure.z))
        .collect();
    assert_eq!(
        actual, expected,
        "structures must land around the anchor, not at their original world coordinates"
    );
}

/// Merging adds to an existing base: no new base camp, no second Pal Box, and
/// the target base gains exactly the blueprint's remaining structures.
#[test]
fn merging_adds_structures_to_the_target_base_without_founding_one() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let owner = common::fixture_player_uid(&session);
    let pal_boxes = blueprint
        .structures
        .iter()
        .filter(|structure| structure.map_object_id == "PalBoxV2")
        .count();
    assert_eq!(pal_boxes, 1, "the fixture base has exactly one Pal Box");
    let bases_before = common::base_count(&session);
    let structures_before = placed_structures(&session, base_id).len();
    assert!(
        structures_before > 1,
        "the target base must already hold structures"
    );

    let mut anchor = common::fixture_base_anchor(&session, base_id);
    anchor.x += 20_000.0;
    let result = place::place(
        &mut session,
        &blueprint,
        &merge_request(anchor, base_id, owner),
        &game_data(),
    )
    .expect("merge placement");

    assert_eq!(result.base_id, None, "a merge founds no base");
    assert_eq!(
        common::base_count(&session),
        bases_before,
        "a merge adds no base camp"
    );
    assert_eq!(
        result.structures_placed as usize,
        blueprint.structures.len() - pal_boxes,
        "a merge drops the blueprint's own Pal Box"
    );
    assert_eq!(
        placed_structures(&session, base_id).len(),
        structures_before + blueprint.structures.len() - pal_boxes,
        "the target base must gain every merged structure"
    );
}

#[test]
fn a_placed_blueprint_still_serializes_and_parses_back() {
    let mut session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");
    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);
    assert!(
        !blueprint.characters.is_empty() && !blueprint.item_containers.is_empty(),
        "a full capture must carry pals and containers, or this proves nothing about them"
    );

    place::place(
        &mut session,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &game_data(),
    )
    .expect("placement");

    let bytes = session
        .level_sav_bytes()
        .expect("level must serialize after placement");
    let reparsed = psp_core::savio::read_sav_bytes(&bytes)
        .expect("level written after placement must parse back");
    let reloaded = psp_core::session::SaveSession::new_for_tests(
        psp_core::session::SaveKind::InMemory,
        reparsed,
    );
    assert_eq!(
        common::base_count(&reloaded),
        common::base_count(&session),
        "the placed base must survive a serialize/parse round trip"
    );
    assert_eq!(
        common::all_map_object_instance_ids(&reloaded),
        common::all_map_object_instance_ids(&session),
        "every placed structure must survive a serialize/parse round trip"
    );
}

// ---- placing into another save ----

/// The first `GroupSaveDataMap` entry that is a decodable guild. A cross-save
/// placement has to name a guild in the DESTINATION, which shares no id with the
/// save the blueprint came from.
fn first_guild_id(session: &SaveSession) -> Uuid {
    use psp_core::domain::{guild_tail, world};

    world::group_map(&session.level)
        .expect("group map")
        .iter()
        .find(|entry| {
            guild_tail::entry_group_type(entry).as_deref() == Some("EPalGroupType::Guild")
                && guild_tail::entry_group_data(entry)
                    .and_then(guild_tail::as_guild)
                    .is_some()
        })
        .and_then(|entry| psp_core::props::as_uuid(&entry.key))
        .expect("every save fixture must carry a decodable guild")
}

/// A member of `guild_id` -- the natural owner for a placement made into it.
fn guild_member_uid(session: &SaveSession, guild_id: Uuid) -> Uuid {
    use psp_core::domain::{guild_tail, world};

    let entry = world::group_map(&session.level)
        .expect("group map")
        .iter()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(guild_id))
        .expect("the named guild has a GroupSaveDataMap entry");
    let group_data = guild_tail::entry_group_data(entry).expect("guild group data");
    let guild = guild_tail::as_guild(group_data).expect("group is a guild");
    *guild_tail::guild_player_uids(guild)
        .first()
        .expect("the guild must have a member")
}

fn base_camp_entry(session: &SaveSession, base_id: Uuid) -> &psp_core::ue::MapEntry {
    psp_core::domain::world::base_camp_map(&session.level)
        .expect("base camp map")
        .expect("the save must have BaseCampSaveData")
        .iter()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(base_id))
        .unwrap_or_else(|| panic!("no base camp entry for {base_id}"))
}

fn base_camp_raw(
    session: &SaveSession,
    base_id: Uuid,
) -> psp_core::ue::games::palworld::PalBaseCamp {
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let entry = base_camp_entry(session, base_id);
    let value_props = psp_core::props::struct_props(&entry.value).expect("base camp value");
    match value_props.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) => (**raw).clone(),
        _ => panic!("base camp {base_id} carries no typed RawData"),
    }
}

/// The base camp's `WorkerDirector`, decoded from the opaque byte blob uesave
/// hands back untyped -- where the worker container id and the worker spawn
/// point live.
fn worker_director(session: &SaveSession, base_id: Uuid) -> psp_core::palbin::WorkerDirector {
    let entry = base_camp_entry(session, base_id);
    let value_props = psp_core::props::struct_props(&entry.value).expect("base camp value");
    let bytes = psp_core::props::get(value_props, &["WorkerDirector", "RawData"])
        .and_then(psp_core::props::as_byte_array)
        .expect("base camp carries a WorkerDirector blob");
    psp_core::palbin::read_worker_director(bytes).expect("WorkerDirector blob decodes")
}

fn work_collection(session: &SaveSession, base_id: Uuid) -> psp_core::palbin::WorkCollection {
    let entry = base_camp_entry(session, base_id);
    let value_props = psp_core::props::struct_props(&entry.value).expect("base camp value");
    let bytes = psp_core::props::get(value_props, &["WorkCollection", "RawData"])
        .and_then(psp_core::props::as_byte_array)
        .expect("base camp carries a WorkCollection blob");
    psp_core::palbin::read_work_collection(bytes).expect("WorkCollection blob decodes")
}

/// `(individual_character_handle_ids, base_ids, map_object_instance_ids_base_camp_points)`
/// as plain id lists -- the three registers a placement has to enter itself in.
fn guild_lists(session: &SaveSession, guild_id: Uuid) -> (Vec<Uuid>, Vec<Uuid>, Vec<Uuid>) {
    use psp_core::domain::{guild_tail, world};

    let entry = world::group_map(&session.level)
        .expect("group map")
        .iter()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(guild_id))
        .expect("the named guild has a GroupSaveDataMap entry");
    let group_data = guild_tail::entry_group_data(entry).expect("guild group data");
    let guild = guild_tail::as_guild(group_data).expect("group is a guild");
    (
        group_data
            .individual_character_handle_ids
            .iter()
            .map(|handle| psp_core::props::guid_to_uuid(&handle.instance_id))
            .collect(),
        guild
            .base_ids
            .iter()
            .map(psp_core::props::guid_to_uuid)
            .collect(),
        guild
            .map_object_instance_ids_base_camp_points
            .iter()
            .map(psp_core::props::guid_to_uuid)
            .collect(),
    )
}

/// Every `WorkSaveData` element's `(own id, base_camp_id_belong_to)`.
fn work_bindings(session: &SaveSession) -> Vec<(Uuid, Uuid)> {
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    psp_core::domain::world::work_values(&session.level)
        .expect("work values")
        .expect("the save must have WorkSaveData")
        .iter()
        .filter_map(|value| {
            let StructValue::Struct(work_props) = value else {
                return None;
            };
            let Some(Property::Struct(StructValue::Game(PalStruct::Work(raw)))) =
                work_props.0.get(&PropertyKey::from("RawData"))
            else {
                return None;
            };
            let base = raw.base_data.as_ref()?;
            Some((
                psp_core::props::guid_to_uuid(&base.id),
                psp_core::props::guid_to_uuid(&base.base_camp_id_belong_to),
            ))
        })
        .collect()
}

/// Every `(container id, slot player_uid)` pair in `CharacterContainerSaveData`,
/// one per slot.
fn character_container_slot_owners(session: &SaveSession) -> Vec<(Uuid, Uuid)> {
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let mut owners = Vec::new();
    for entry in
        psp_core::domain::world::character_container_map(&session.level).expect("containers")
    {
        let Some(container_id) = capture::container_entry_id(entry) else {
            continue;
        };
        let Some(value_props) = psp_core::props::struct_props(&entry.value) else {
            continue;
        };
        let Some(slots) =
            psp_core::props::get(value_props, &["Slots"]).and_then(psp_core::props::struct_values)
        else {
            continue;
        };
        for slot in slots {
            let StructValue::Struct(slot_props) = slot else {
                continue;
            };
            if let Some(Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw)))) =
                slot_props.0.get(&PropertyKey::from("RawData"))
            {
                owners.push((container_id, psp_core::props::guid_to_uuid(&raw.player_uid)));
            }
        }
    }
    owners
}

/// Every `CharacterContainer` module's `target_container_id` on one structure,
/// walked through the public property surface.
fn character_container_module_ids(properties: &psp_core::ue::Properties) -> Vec<Uuid> {
    use psp_core::ue::games::palworld::PalMapConcreteModelModuleData;
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let mut ids = Vec::new();
    let Some(concrete) = properties
        .0
        .get(&PropertyKey::from("ConcreteModel"))
        .and_then(psp_core::props::struct_props)
    else {
        return ids;
    };
    let Some(modules) = concrete
        .0
        .get(&PropertyKey::from("ModuleMap"))
        .and_then(psp_core::props::map_entries)
    else {
        return ids;
    };
    for module in modules {
        let Some(module_props) = psp_core::props::struct_props(&module.value) else {
            continue;
        };
        if let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModelModule(raw)))) =
            module_props.0.get(&PropertyKey::from("RawData"))
        {
            if let PalMapConcreteModelModuleData::CharacterContainer {
                target_container_id,
                ..
            } = &raw.data
            {
                ids.push(psp_core::props::guid_to_uuid(target_container_id));
            }
        }
    }
    ids
}

fn character_container_entry(session: &SaveSession, container_id: Uuid) -> &psp_core::ue::MapEntry {
    psp_core::domain::world::character_container_map(&session.level)
        .expect("containers")
        .iter()
        .find(|entry| capture::container_entry_id(entry) == Some(container_id))
        .unwrap_or_else(|| panic!("no character container entry for {container_id}"))
}

/// Every `(work id, individual id)` pair a `WorkSaveData` entry names -- the
/// assigned worker on the work itself and on each `WorkAssignMap` entry. Nil
/// ids ("nobody is on this") are left out.
fn work_individual_refs(session: &SaveSession) -> Vec<(Uuid, Uuid)> {
    use psp_core::ue::games::palworld::PalWorkTypeSpecificData;
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let mut refs = Vec::new();
    let Some(values) = psp_core::domain::world::work_values(&session.level).expect("work values")
    else {
        return refs;
    };
    for value in values {
        let StructValue::Struct(work_props) = value else {
            continue;
        };
        let Some(Property::Struct(StructValue::Game(PalStruct::Work(raw)))) =
            work_props.0.get(&PropertyKey::from("RawData"))
        else {
            continue;
        };
        let work_id = raw
            .base_data
            .as_ref()
            .map(|base| psp_core::props::guid_to_uuid(&base.id))
            .unwrap_or_default();
        match &raw.work_specific_data {
            PalWorkTypeSpecificData::Assign {
                assigned_individual_id,
                ..
            } => {
                refs.push((
                    work_id,
                    psp_core::props::guid_to_uuid(&assigned_individual_id.instance_id),
                ));
            }
            PalWorkTypeSpecificData::ReviveCharacter {
                target_individual_id,
            } => {
                refs.push((
                    work_id,
                    psp_core::props::guid_to_uuid(&target_individual_id.instance_id),
                ));
            }
            _ => {}
        }
        if let Some(Property::Map(entries)) = work_props.0.get(&PropertyKey::from("WorkAssignMap"))
        {
            for entry in entries {
                let Some(assign_props) = psp_core::props::struct_props(&entry.value) else {
                    continue;
                };
                if let Some(Property::Struct(StructValue::Game(PalStruct::WorkAssign(assign)))) =
                    assign_props.0.get(&PropertyKey::from("RawData"))
                {
                    refs.push((
                        work_id,
                        psp_core::props::guid_to_uuid(&assign.assigned_individual_id.instance_id),
                    ));
                }
            }
        }
    }
    refs.retain(|(_, individual)| !individual.is_nil());
    refs
}

/// Every `(work id, individual id)` a work names that `CharacterSaveParameterMap`
/// has no entry for -- the dangling reference a placement must never create.
fn works_naming_a_missing_character(session: &SaveSession) -> Vec<(Uuid, Uuid)> {
    let characters = character_instance_ids(session);
    work_individual_refs(session)
        .into_iter()
        .filter(|(_, individual)| !characters.contains(individual))
        .collect()
}

fn character_container_ids(session: &SaveSession) -> BTreeSet<Uuid> {
    psp_core::domain::world::character_container_map(&session.level)
        .expect("containers")
        .iter()
        .filter_map(capture::container_entry_id)
        .collect()
}

fn character_instance_ids(session: &SaveSession) -> BTreeSet<Uuid> {
    psp_core::domain::world::character_map(&session.level)
        .expect("characters")
        .iter()
        .filter_map(psp_core::domain::world::entry_instance_id)
        .collect()
}

/// The `v1_relics` base every cross-save test ships, at one capture layer.
fn source_blueprint(source: &SaveSession, options: CaptureOptions) -> BaseBlueprint {
    capture::capture(source, common::fixture_base_id(source), options, "Home").expect("capture")
}

/// Moving a base into ANOTHER save is what a blueprint is for, and it is exactly
/// where uesave's write schemas run out: a schema is recorded only for a
/// property that was actually READ, so every property the blueprint introduces
/// has no tag in a destination that never carried one and `level_sav_bytes`
/// fails -- after the placement has already been applied, leaving the user
/// unable to save at all.
///
/// All three capture layers are covered because each brings different
/// properties: `configured` adds structure condition and access config, `full`
/// adds pals, containers and dynamic items on top.
#[test]
fn a_blueprint_placed_into_another_save_leaves_that_save_writable() {
    let source = common::load_fixture_session("v1_relics");

    for (layer, options) in [
        ("blueprint", CaptureOptions::blueprint()),
        ("configured", CaptureOptions::configured()),
        ("full", CaptureOptions::full()),
    ] {
        let blueprint = source_blueprint(&source, options);
        assert!(
            !blueprint.structures.is_empty() && !blueprint.works.is_empty(),
            "{layer}: the blueprint must carry structures and works"
        );
        if options.worker_pals {
            assert!(
                !blueprint.characters.is_empty()
                    && !blueprint.item_containers.is_empty()
                    && !blueprint.dynamic_items.is_empty(),
                "{layer}: a full capture must carry pals, containers and items, or the layers \
                 are indistinguishable"
            );
        }

        for target_name in ["v1_stats", "world1"] {
            let mut target = common::load_fixture_session(target_name);
            assert!(
                target.level_sav_bytes().is_ok(),
                "{target_name} must serialize BEFORE the placement, or a failure after it \
                 proves nothing about the placement"
            );
            let guild_id = first_guild_id(&target);
            let owner = guild_member_uid(&target, guild_id);
            let bases_before = common::base_count(&target);
            let objects_before = common::all_map_object_instance_ids(&target).len();

            let result = place::place(
                &mut target,
                &blueprint,
                &new_base_request(anchor_far_from_everything(), guild_id, owner),
                &game_data(),
            )
            .unwrap_or_else(|error| panic!("{layer} -> {target_name}: placement failed: {error}"));
            assert_eq!(
                result.structures_placed as usize,
                blueprint.structures.len(),
                "{layer} -> {target_name}: every structure must be placed"
            );

            let bytes = target.level_sav_bytes().unwrap_or_else(|error| {
                panic!("{layer} -> {target_name}: the destination must still serialize: {error}")
            });
            let reparsed = psp_core::savio::read_sav_bytes(&bytes).unwrap_or_else(|error| {
                panic!("{layer} -> {target_name}: the written level must parse back: {error}")
            });
            let reloaded =
                SaveSession::new_for_tests(psp_core::session::SaveKind::InMemory, reparsed);
            assert_eq!(
                common::base_count(&reloaded),
                bases_before + 1,
                "{layer} -> {target_name}: the placed base must survive the round trip"
            );
            assert_eq!(
                common::all_map_object_instance_ids(&reloaded).len(),
                objects_before + blueprint.structures.len(),
                "{layer} -> {target_name}: every placed structure must survive the round trip"
            );
        }
    }
}

/// `world2` has never held a work or a base camp, so those arrays are absent
/// from its world tree entirely. The refusal has to come before anything is
/// written -- and "nothing was written" is checked against the destination's own
/// bytes, not merely against a count.
#[test]
fn a_destination_missing_a_collection_is_refused_before_anything_lands() {
    let source = common::load_fixture_session("v1_relics");
    let blueprint = source_blueprint(&source, CaptureOptions::full());
    assert!(
        !blueprint.works.is_empty(),
        "the blueprint must carry works"
    );

    let mut target = common::load_fixture_session("world2");
    let bytes_before = target
        .level_sav_bytes()
        .expect("world2 must serialize at baseline");
    let fingerprint_before = session_fingerprint(&target);
    let guild_id = first_guild_id(&target);
    let owner = guild_member_uid(&target, guild_id);

    let error = place::place(
        &mut target,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &game_data(),
    )
    .expect_err("a destination with no WorkSaveData must refuse the placement");

    assert!(
        error.to_string().contains("WorkSaveData"),
        "the refusal must name the missing collection: {error}"
    );
    assert_eq!(
        session_fingerprint(&target),
        fingerprint_before,
        "a refused placement must not half-apply"
    );
    assert_eq!(
        target
            .level_sav_bytes()
            .expect("world2 must still serialize"),
        bytes_before,
        "a refused placement must leave the destination byte-identical"
    );
}

/// Adds a byte to one of a base camp's opaque blobs. `WorkerDirector` is a
/// fixed 118-byte layout and `WorkCollection` is checked to its last byte, so
/// one byte too many makes either refuse to decode -- which is what a Palworld
/// update that moved a field would look like from here.
fn lengthen_base_camp_blob(base_camp: &mut psp_core::ue::Properties, field: &str) {
    let raw_data = psp_core::props::get_mut(base_camp, &[field, "RawData"])
        .unwrap_or_else(|| panic!("the base camp must carry a {field} blob"));
    let bytes = psp_core::props::as_byte_array_mut(raw_data)
        .unwrap_or_else(|| panic!("{field} RawData must be a byte array"));
    bytes.push(0);
}

fn lengthen_session_base_camp_blob(session: &mut SaveSession, base_id: Uuid, field: &str) {
    let entries = psp_core::domain::world::base_camp_map_mut(&mut session.level)
        .expect("base camp map")
        .expect("the fixture must have BaseCampSaveData");
    let entry = entries
        .iter_mut()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(base_id))
        .expect("the named base camp entry exists");
    let value_props = psp_core::props::struct_props_mut(&mut entry.value).expect("base camp value");
    lengthen_base_camp_blob(value_props, field);
}

/// The blueprint's own `WorkerDirector` and `WorkCollection` are the two blobs
/// a placement has to rewrite before it can call the base its own. Undecodable,
/// they used to be carried over verbatim and the placement reported success:
/// the landed base's workers would resolve to the SOURCE save's container and
/// its `WorkCollection` would name the source save's works, under the source
/// base's id. Neither blob is modelled by uesave, so a Palworld update that
/// moved a field would reinstate exactly that, on every placement.
#[test]
fn a_blueprint_blob_that_does_not_decode_refuses_the_placement() {
    let source = common::load_fixture_session("v1_relics");
    let intact = source_blueprint(&source, CaptureOptions::full());

    // The control: this blueprint, blobs intact, places cleanly into this
    // destination -- so a refusal below is the corruption and nothing else.
    {
        let mut target = common::load_fixture_session("world1");
        let guild_id = first_guild_id(&target);
        let owner = guild_member_uid(&target, guild_id);
        place::place(
            &mut target,
            &intact,
            &new_base_request(anchor_far_from_everything(), guild_id, owner),
            &game_data(),
        )
        .expect("setup: the intact blueprint must place, or the refusals below prove nothing");
    }

    for field in ["WorkerDirector", "WorkCollection"] {
        let mut blueprint = intact.clone();
        lengthen_base_camp_blob(
            blueprint
                .base_camp
                .as_mut()
                .expect("the captured base has a base camp"),
            field,
        );

        let mut target = common::load_fixture_session("world1");
        let bytes_before = target
            .level_sav_bytes()
            .expect("world1 must serialize at baseline");
        let fingerprint_before = session_fingerprint(&target);
        let guild_id = first_guild_id(&target);
        let owner = guild_member_uid(&target, guild_id);

        let error = match place::place(
            &mut target,
            &blueprint,
            &new_base_request(anchor_far_from_everything(), guild_id, owner),
            &game_data(),
        ) {
            Err(error) => error,
            Ok(result) => panic!(
                "{field}: a blob that does not decode must refuse the placement, got {result:?}"
            ),
        };
        assert!(
            error.to_string().contains(field),
            "{field}: the refusal must name the blob that failed: {error}"
        );
        assert_eq!(
            session_fingerprint(&target),
            fingerprint_before,
            "{field}: a refused placement must not half-apply"
        );
        assert_eq!(
            target
                .level_sav_bytes()
                .expect("world1 must still serialize"),
            bytes_before,
            "{field}: a refused placement must leave the destination byte-identical"
        );
    }
}

/// The fourth blob is the TARGET base's `WorkCollection`, which a merge appends
/// its works to. Undecodable, the append used to be skipped and the merge
/// reported success, leaving the target base holding works it does not list.
/// The refusal has to land before anything is written, so the blob is decoded
/// during preflight rather than at the point of the append, which runs from
/// inside `commit`.
#[test]
fn a_merge_into_a_base_whose_work_collection_is_corrupt_lands_nothing() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let owner = common::fixture_player_uid(&session);
    assert!(
        !blueprint.works.is_empty(),
        "the blueprint must carry works to append"
    );

    let mut anchor = common::fixture_base_anchor(&session, base_id);
    anchor.x += 20_000.0;

    // The control: the same merge succeeds while the target's blob is intact.
    {
        let mut control = session_with_limits();
        place::place(
            &mut control,
            &blueprint,
            &merge_request(anchor, base_id, owner),
            &game_data(),
        )
        .expect("setup: the merge must succeed against an intact target, or this proves nothing");
    }

    lengthen_session_base_camp_blob(&mut session, base_id, "WorkCollection");
    let bytes_before = session
        .level_sav_bytes()
        .expect("the target must serialize at baseline");
    let fingerprint_before = session_fingerprint(&session);

    let error = match place::place(
        &mut session,
        &blueprint,
        &merge_request(anchor, base_id, owner),
        &game_data(),
    ) {
        Err(error) => error,
        Ok(result) => {
            panic!("a target WorkCollection that does not decode must refuse the merge, got {result:?}")
        }
    };
    assert!(
        error.to_string().contains("WorkCollection"),
        "the refusal must name the blob that failed: {error}"
    );
    assert_eq!(
        session_fingerprint(&session),
        fingerprint_before,
        "a refused merge must not half-apply"
    );
    assert_eq!(
        session
            .level_sav_bytes()
            .expect("the target must still serialize"),
        bytes_before,
        "a refused merge must leave the target byte-identical"
    );
}

// ---- what a placement rebinds ----

/// Everything `stage_identity` rebinds, pinned in one placement made into a
/// destination that shares no guild, no owner and no base with the save the
/// blueprint came from -- so none of these values can match by coincidence.
#[test]
fn a_placed_base_rebinds_every_object_it_brings() {
    let source = common::load_fixture_session("v1_relics");
    let source_base = common::fixture_base_id(&source);
    let blueprint = source_blueprint(&source, CaptureOptions::full());
    let source_guild = common::fixture_guild_id(&source);

    let mut target = common::load_fixture_session("world1");
    let guild_id = first_guild_id(&target);
    let owner = guild_member_uid(&target, guild_id);
    assert_ne!(
        guild_id, source_guild,
        "the destination guild must differ from the source's"
    );
    assert!(!guild_id.is_nil(), "the destination guild id must be real");
    assert!(!owner.is_nil(), "the destination owner uid must be real");

    let anchor = anchor_far_from_everything();
    let works_before: BTreeSet<Uuid> = work_bindings(&target)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let containers_before = character_container_ids(&target);
    let characters_before = character_instance_ids(&target);
    // A captured pal names no guild at all -- the scrub pass zeroes it so the
    // source save's guild cannot travel in a shared blueprint -- so a placement
    // that forgot to rebind it would leave nil behind rather than the
    // destination's guild.
    assert!(
        !blueprint.characters.is_empty(),
        "the blueprint must carry pals to rebind"
    );
    for entry in &blueprint.characters {
        assert_eq!(
            psp_core::domain::world::entry_character_data(entry)
                .map(|data| psp_core::props::guid_to_uuid(&data.group_id)),
            Some(Uuid::nil()),
            "a captured pal must name no guild"
        );
    }

    let result = place::place(
        &mut target,
        &blueprint,
        &new_base_request(anchor, guild_id, owner),
        &game_data(),
    )
    .expect("placement");
    let placed_base = result.base_id.expect("a new base must report its id");

    let placed_raw = base_camp_raw(&target, placed_base);
    let source_raw = base_camp_raw(&source, source_base);
    assert_ne!(
        (
            source_raw.transform.translation.x.0,
            source_raw.transform.translation.y.0
        ),
        (anchor.x, anchor.y),
        "the anchor must differ from the source base's own position"
    );
    assert_eq!(
        (
            placed_raw.transform.translation.x.0,
            placed_raw.transform.translation.y.0,
            placed_raw.transform.translation.z.0
        ),
        (anchor.x, anchor.y, anchor.z),
        "the placed base camp must sit at the anchor"
    );
    assert_eq!(
        psp_core::props::guid_to_uuid(&placed_raw.group_id_belong_to),
        guild_id,
        "the placed base camp must belong to the target guild"
    );
    assert_eq!(
        psp_core::props::guid_to_uuid(&placed_raw.id),
        placed_base,
        "the placed base camp must carry the id the placement reported"
    );

    let structures = placed_structures(&target, placed_base);
    let base_camp_points: Vec<Uuid> = structures
        .iter()
        .filter_map(|structure| structure.base_camp_point_id)
        .collect();
    assert_eq!(
        base_camp_points.len(),
        1,
        "exactly one placed structure is a BaseCampPoint"
    );
    assert_eq!(
        base_camp_points[0], placed_base,
        "the BaseCampPoint must name the new base, not the one it was captured from"
    );

    let placed_works: Vec<(Uuid, Uuid)> = work_bindings(&target)
        .into_iter()
        .filter(|(id, _)| !works_before.contains(id))
        .collect();
    assert_eq!(
        placed_works.len(),
        blueprint.works.len(),
        "every blueprint work must reach WorkSaveData"
    );
    for (work_id, base) in &placed_works {
        assert_eq!(
            *base, placed_base,
            "work {work_id} must belong to the new base"
        );
    }

    let placed_containers: BTreeSet<Uuid> = character_container_ids(&target)
        .difference(&containers_before)
        .copied()
        .collect();
    assert_eq!(
        placed_containers.len(),
        blueprint.character_containers.len(),
        "every captured character container must land"
    );
    let placed_slot_owners: Vec<Uuid> = character_container_slot_owners(&target)
        .into_iter()
        .filter(|(container, _)| placed_containers.contains(container))
        .map(|(_, slot_owner)| slot_owner)
        .collect();
    assert!(
        !placed_slot_owners.is_empty(),
        "the placed containers must have slots, or slot ownership is untested"
    );
    for slot_owner in &placed_slot_owners {
        assert_eq!(
            *slot_owner, owner,
            "every placed container slot must name the owner"
        );
    }

    let placed_characters: BTreeSet<Uuid> = character_instance_ids(&target)
        .difference(&characters_before)
        .copied()
        .collect();
    assert_eq!(
        placed_characters.len(),
        blueprint.characters.len(),
        "every captured pal must land"
    );
    for entry in psp_core::domain::world::character_map(&target.level).expect("characters") {
        let Some(instance_id) = psp_core::domain::world::entry_instance_id(entry) else {
            continue;
        };
        if !placed_characters.contains(&instance_id) {
            continue;
        }
        assert_eq!(
            psp_core::domain::world::entry_character_data(entry)
                .map(|data| psp_core::props::guid_to_uuid(&data.group_id)),
            Some(guild_id),
            "placed pal {instance_id} must join the target guild"
        );
        assert_eq!(
            psp_core::domain::world::entry_save_parameter(entry)
                .and_then(|params| psp_core::props::get(params, &["OwnerPlayerUId"]))
                .and_then(psp_core::props::as_uuid),
            Some(owner),
            "placed pal {instance_id} must answer to the placing player"
        );
    }
}

/// The guild registers a placement three times over, and none of the three
/// follows from the structures landing: without them the base exists but no
/// guild owns it, its Pal Box never shows up as a base camp point, and its pals
/// belong to nobody.
#[test]
fn a_new_base_registers_itself_with_its_guild() {
    let source = common::load_fixture_session("v1_relics");
    let blueprint = source_blueprint(&source, CaptureOptions::full());

    let mut target = common::load_fixture_session("world1");
    let guild_id = first_guild_id(&target);
    let owner = guild_member_uid(&target, guild_id);
    let (handles_before, base_ids_before, camp_points_before) = guild_lists(&target, guild_id);
    let characters_before = character_instance_ids(&target);

    let result = place::place(
        &mut target,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &game_data(),
    )
    .expect("placement");
    let placed_base = result.base_id.expect("a new base must report its id");
    let (handles_after, base_ids_after, camp_points_after) = guild_lists(&target, guild_id);

    assert_eq!(
        base_ids_after.len(),
        base_ids_before.len() + 1,
        "the guild must gain exactly one base"
    );
    assert!(
        base_ids_after.contains(&placed_base),
        "the guild's base_ids must name the placed base"
    );

    let pal_box: Vec<Uuid> = placed_structures(&target, placed_base)
        .into_iter()
        .filter(|structure| structure.map_object_id == "PalBoxV2")
        .map(|structure| structure.instance_id)
        .collect();
    assert_eq!(pal_box.len(), 1, "the placed base has exactly one Pal Box");
    assert_eq!(
        camp_points_after.len(),
        camp_points_before.len() + 1,
        "the guild must gain exactly one base camp point"
    );
    assert!(
        camp_points_after.contains(&pal_box[0]),
        "the guild's base camp points must name the PLACED Pal Box instance"
    );

    let placed_characters: BTreeSet<Uuid> = character_instance_ids(&target)
        .difference(&characters_before)
        .copied()
        .collect();
    assert!(
        !placed_characters.is_empty(),
        "a full capture must bring pals"
    );
    let gained: BTreeSet<Uuid> = handles_after[handles_before.len()..]
        .iter()
        .copied()
        .collect();
    assert_eq!(
        handles_after.len(),
        handles_before.len() + placed_characters.len(),
        "the guild must gain one character handle per placed pal"
    );
    assert_eq!(
        gained, placed_characters,
        "the handles the guild gained must be exactly the pals that landed"
    );
}

/// A base names its works twice: once per `WorkSaveData` entry, and once more in
/// the opaque `WorkCollection` blob on the base camp. A merge that appends to
/// the first and not the second leaves the target base unable to find the work
/// it just gained.
#[test]
fn merging_appends_its_works_to_the_target_bases_work_collection() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint = blueprint_of(&session, base_id);
    let owner = common::fixture_player_uid(&session);
    assert!(
        !blueprint.works.is_empty(),
        "the blueprint must carry works"
    );

    let works_before: BTreeSet<Uuid> = work_bindings(&session)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let collection_before = work_collection(&session, base_id);
    assert!(
        !collection_before.work_ids.is_empty(),
        "the target base must already name works, or 'appends' is untested"
    );

    let mut anchor = common::fixture_base_anchor(&session, base_id);
    anchor.x += 20_000.0;
    place::place(
        &mut session,
        &blueprint,
        &merge_request(anchor, base_id, owner),
        &game_data(),
    )
    .expect("merge placement");

    let merged_works: Vec<Uuid> = work_bindings(&session)
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| !works_before.contains(id))
        .collect();
    assert_eq!(
        merged_works.len(),
        blueprint.works.len(),
        "every blueprint work must reach WorkSaveData"
    );

    let collection_after = work_collection(&session, base_id);
    assert_eq!(
        collection_after.work_ids.len(),
        collection_before.work_ids.len() + merged_works.len(),
        "the target base's WorkCollection must grow by the merged work count"
    );
    assert_eq!(
        collection_after.work_ids[..collection_before.work_ids.len()],
        collection_before.work_ids[..],
        "a merge must not disturb the works the target base already had"
    );
    assert_eq!(
        collection_after.work_ids[collection_before.work_ids.len()..],
        merged_works[..],
        "the ids appended to the WorkCollection must be the works that landed"
    );
}

/// `WorldCaches` maps ids to POSITIONS in the world tree and a placement appends
/// to every map it indexes, so a placement that leaves the caches standing
/// leaves later lookups resolving against a tree that no longer matches.
#[test]
fn placing_invalidates_the_world_lookup_caches() {
    use psp_core::domain::world;

    let source = common::load_fixture_session("v1_relics");
    let blueprint = source_blueprint(&source, CaptureOptions::full());

    let mut target = common::load_fixture_session("world1");
    let guild_id = first_guild_id(&target);
    let owner = guild_member_uid(&target, guild_id);
    target.caches.character_index = Some(world::build_character_index(&target.level));
    target.caches.item_container_index = Some(world::build_item_container_index(&target.level));
    target.caches.character_container_index =
        Some(world::build_character_container_index(&target.level));
    let stale_containers = target
        .caches
        .character_container_index
        .clone()
        .expect("just built");
    let containers_before = character_container_ids(&target);

    place::place(
        &mut target,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &game_data(),
    )
    .expect("placement");

    // Why holding on to the index is a defect and not a missed optimisation: it
    // cannot answer for anything that just landed.
    let placed: Vec<Uuid> = character_container_ids(&target)
        .difference(&containers_before)
        .copied()
        .collect();
    assert!(
        !placed.is_empty(),
        "a full capture must bring character containers"
    );
    for container_id in &placed {
        assert!(
            !stale_containers.contains_key(container_id),
            "the pre-placement index cannot know about {container_id}"
        );
    }

    assert!(
        target.caches.character_index.is_none(),
        "character index must be dropped"
    );
    assert!(
        target.caches.item_container_index.is_none(),
        "item container index must be dropped"
    );
    assert!(
        target.caches.character_container_index.is_none(),
        "character container index must be dropped"
    );
}

// ---- the base camp's opaque WorkerDirector blob ----

/// The `WorkerDirector` is a raw byte blob, so nothing in the typed remap
/// touches it. Carried over verbatim it still names the SOURCE save's worker
/// container -- an id that does not exist in another save at all, and that in
/// the same save resolves to the pals of the base the blueprint was captured
/// from -- and still sends the base's workers to the source base's coordinates.
#[test]
fn the_placed_bases_worker_director_is_retargeted() {
    let source = common::load_fixture_session("v1_relics");
    let source_base = common::fixture_base_id(&source);
    let blueprint = source_blueprint(&source, CaptureOptions::full());

    // Read off the source fixture, so every expectation below is independent of
    // the placement.
    let source_director = worker_director(&source, source_base);
    let source_transform = base_camp_raw(&source, source_base).transform;
    assert!(
        !source_director.container_id.is_nil(),
        "the source base must name a worker container"
    );
    let source_offset = (
        source_director.spawn_transform.translation.x.0 - source_transform.translation.x.0,
        source_director.spawn_transform.translation.y.0 - source_transform.translation.y.0,
        source_director.spawn_transform.translation.z.0 - source_transform.translation.z.0,
    );
    let source_radius =
        (source_offset.0 * source_offset.0 + source_offset.1 * source_offset.1).sqrt();
    assert!(
        source_radius > 1.0,
        "the source spawn point must be offset from its base, or re-basing is untestable"
    );

    // Both directions matter: in the same save a stale container id silently
    // resolves to the WRONG pals, in another save it resolves to nothing at all.
    for target_name in ["v1_relics", "world1"] {
        let mut target = common::load_fixture_session(target_name);
        let guild_id = first_guild_id(&target);
        let owner = guild_member_uid(&target, guild_id);
        let anchor = anchor_far_from_everything();
        let containers_before = character_container_ids(&target);

        let result = place::place(
            &mut target,
            &blueprint,
            &new_base_request(anchor, guild_id, owner),
            &game_data(),
        )
        .unwrap_or_else(|error| panic!("{target_name}: placement failed: {error}"));
        let placed_base = result.base_id.expect("a new base must report its id");
        let director = worker_director(&target, placed_base);

        assert_eq!(
            director.id, placed_base,
            "{target_name}: the director must name the base it now belongs to"
        );

        let placed_containers: BTreeSet<Uuid> = character_container_ids(&target)
            .difference(&containers_before)
            .copied()
            .collect();
        assert_ne!(
            director.container_id, source_director.container_id,
            "{target_name}: the director must not still name the source save's worker container"
        );
        assert!(
            placed_containers.contains(&director.container_id),
            "{target_name}: the director must name one of the containers this placement \
             inserted, got {}",
            director.container_id
        );
        assert_eq!(
            psp_core::domain::guild::base_guild_and_container(base_camp_entry(
                &target,
                placed_base
            )),
            Some((guild_id, director.container_id)),
            "{target_name}: the app's own base -> worker container lookup must agree"
        );

        // The spawn point is a WORLD position, so it has to be re-based onto the
        // anchor. A yaw rotation about Z preserves both the vertical offset and
        // the horizontal distance, so the two together pin the re-basing without
        // re-deriving the rotation the placement applies.
        let spawn = &director.spawn_transform.translation;
        let placed_offset = (
            spawn.x.0 - anchor.x,
            spawn.y.0 - anchor.y,
            spawn.z.0 - anchor.z,
        );
        let placed_radius =
            (placed_offset.0 * placed_offset.0 + placed_offset.1 * placed_offset.1).sqrt();
        assert!(
            (placed_radius - source_radius).abs() < 1e-6,
            "{target_name}: the spawn point must keep its distance from the base camp, \
             {placed_radius} vs {source_radius}"
        );
        assert!(
            (placed_offset.2 - source_offset.2).abs() < 1e-6,
            "{target_name}: a yaw rotation cannot change the spawn point's height above the \
             base camp, {} vs {}",
            placed_offset.2,
            source_offset.2
        );
        assert!(
            (spawn.x.0 - source_director.spawn_transform.translation.x.0).abs() > 1.0,
            "{target_name}: the spawn point must have moved off the source base's coordinates"
        );
    }
}

/// `the_placed_bases_worker_director_is_retargeted` runs `full` only, and only
/// a `full` capture brings a `CharacterContainerSaveData` entry the director can
/// be pointed at. `blueprint` is the DEFAULT layer, so the layer that decides
/// what most placements do is the one that has to be pinned: a base whose
/// director names the nil guid has no worker container at all, and the app's own
/// `base_guild_and_container` lookup says so.
///
/// The container travels emptied rather than absent, so the slot count -- the
/// base's worker capacity -- is the same at every layer and only the occupancy
/// differs.
#[test]
fn every_capture_layer_gives_the_placed_base_a_worker_container() {
    let source = common::load_fixture_session("v1_relics");
    let source_base = common::fixture_base_id(&source);

    // Read off the source fixture, independent of any placement.
    let (_, source_container) =
        psp_core::domain::guild::base_guild_and_container(base_camp_entry(&source, source_base))
            .expect("the fixture base must resolve a worker container");
    let (source_slots, source_pals) =
        common::container_slot_census(character_container_entry(&source, source_container));
    assert!(
        source_slots > 0 && source_pals == source_slots,
        "the fixture's worker container must be full, or an emptied copy is indistinguishable \
         from the original: {source_pals} of {source_slots}"
    );

    for (layer, options) in [
        ("blueprint", CaptureOptions::blueprint()),
        ("configured", CaptureOptions::configured()),
        ("full", CaptureOptions::full()),
    ] {
        let blueprint = source_blueprint(&source, options);
        let expected_pals = if options.worker_pals { source_pals } else { 0 };

        for target_name in ["v1_relics", "v1_stats", "world1"] {
            let mut target = common::load_fixture_session(target_name);
            let guild_id = first_guild_id(&target);
            let owner = guild_member_uid(&target, guild_id);
            let containers_before = character_container_ids(&target);

            let result = place::place(
                &mut target,
                &blueprint,
                &new_base_request(anchor_far_from_everything(), guild_id, owner),
                &game_data(),
            )
            .unwrap_or_else(|error| panic!("{layer} -> {target_name}: placement failed: {error}"));
            let placed_base = result.base_id.expect("a new base must report its id");
            let director = worker_director(&target, placed_base);

            assert!(
                !director.container_id.is_nil(),
                "{layer} -> {target_name}: the placed base's director names no worker container"
            );
            let placed_containers: BTreeSet<Uuid> = character_container_ids(&target)
                .difference(&containers_before)
                .copied()
                .collect();
            assert!(
                placed_containers.contains(&director.container_id),
                "{layer} -> {target_name}: the director must name a container this placement \
                 inserted, got {}",
                director.container_id
            );
            assert_eq!(
                psp_core::domain::guild::base_guild_and_container(base_camp_entry(
                    &target,
                    placed_base
                )),
                Some((guild_id, director.container_id)),
                "{layer} -> {target_name}: the app's own base -> worker container lookup must agree"
            );
            assert_eq!(
                common::container_slot_census(character_container_entry(
                    &target,
                    director.container_id
                )),
                (source_slots, expected_pals),
                "{layer} -> {target_name}: the worker container must keep the base's capacity and \
                 hold only the pals this layer captures"
            );
        }
    }
}

/// A merge founds no base, so it drops the blueprint's base camp -- and with it
/// the `WorkerDirector` that was the only thing naming the base's worker
/// container. An EMPTY container left behind that way is litter nothing can
/// reach, so it goes. One still holding pals stays: `remap` has already pointed
/// the merged works at those pals, so dropping them would land works naming
/// `CharacterSaveParameterMap` keys that do not exist -- and would throw the
/// base's whole workforce away without saying so.
///
/// Both layers are needed to separate the two halves of that rule: `blueprint`
/// carries the worker container empty, so only the drop can happen; `full`
/// carries it holding every worker, so only the keep can.
#[test]
fn a_merge_drops_only_the_container_that_is_both_unnamed_and_empty() {
    for (layer, options, expected_dropped, expected_kept_for_pals) in [
        ("blueprint", CaptureOptions::blueprint(), 1usize, 0usize),
        ("full", CaptureOptions::full(), 0usize, 1usize),
    ] {
        let mut session = session_with_limits();
        let base_id = common::fixture_base_id(&session);
        let blueprint = capture::capture(&session, base_id, options, "Home").expect("capture");
        let owner = common::fixture_player_uid(&session);

        // Derived from the blueprint alone: once the base camp and the Pal Box
        // are gone, a container survives only because a remaining structure's
        // module still names it, or because it holds a pal.
        let reachable: BTreeSet<Uuid> = blueprint
            .structures
            .iter()
            .filter(|structure| structure.map_object_id != "PalBoxV2")
            .flat_map(|structure| character_container_module_ids(&structure.properties))
            .collect();
        let named = |entry: &psp_core::ue::MapEntry| {
            capture::container_entry_id(entry).is_some_and(|id| reachable.contains(&id))
        };
        let expected_kept = blueprint
            .character_containers
            .iter()
            .filter(|entry| named(entry) || common::container_slot_census(entry).1 > 0)
            .count();
        let kept_for_pals = blueprint
            .character_containers
            .iter()
            .filter(|entry| !named(entry) && common::container_slot_census(entry).1 > 0)
            .count();
        assert_eq!(
            blueprint.character_containers.len() - expected_kept,
            expected_dropped,
            "{layer}: the capture must carry {expected_dropped} container(s) a merge can neither \
             reach nor lose pals over, or the drop is untested"
        );
        assert_eq!(
            kept_for_pals, expected_kept_for_pals,
            "{layer}: the capture must carry {expected_kept_for_pals} container(s) kept ONLY \
             because pals are in them, or the keep is untested"
        );

        let containers_before = character_container_ids(&session);
        let mut anchor = common::fixture_base_anchor(&session, base_id);
        anchor.x += 20_000.0;
        place::place(
            &mut session,
            &blueprint,
            &merge_request(anchor, base_id, owner),
            &game_data(),
        )
        .unwrap_or_else(|error| panic!("{layer}: merge placement failed: {error}"));

        let placed_containers: BTreeSet<Uuid> = character_container_ids(&session)
            .difference(&containers_before)
            .copied()
            .collect();
        assert_eq!(
            placed_containers.len(),
            expected_kept,
            "{layer}: a merge must land exactly the containers a structure names or a pal sits in"
        );

        let module_targets: BTreeSet<Uuid> =
            psp_core::domain::world::map_object_values(&session.level)
                .expect("map objects")
                .expect("MapObjectSaveData")
                .iter()
                .filter_map(|value| match value {
                    psp_core::ue::StructValue::Struct(properties) => Some(properties),
                    _ => None,
                })
                .flat_map(character_container_module_ids)
                .collect();
        let landed_unnamed: Vec<Uuid> = placed_containers
            .iter()
            .copied()
            .filter(|id| !module_targets.contains(id))
            .collect();
        assert_eq!(
            landed_unnamed.len(),
            expected_kept_for_pals,
            "{layer}: the only container allowed to land unnamed is one holding pals, got \
             {landed_unnamed:?}"
        );
        for container_id in &landed_unnamed {
            assert!(
                common::container_slot_census(character_container_entry(&session, *container_id)).1
                    > 0,
                "{layer}: container {container_id} landed unnamed AND empty"
            );
        }
    }
}

/// The other half of that rule, where the harm is: a merged `WorkSaveData`
/// entry names its worker by instance id, and `remap` rewrote those names to the
/// blueprint's fresh pal ids before anything decided which pals to land. Land
/// the works without the pals and the target save carries works pointing at
/// `CharacterSaveParameterMap` keys that were never inserted.
#[test]
fn a_merge_lands_its_workers_and_leaves_no_work_naming_a_missing_pal() {
    let mut session = session_with_limits();
    let base_id = common::fixture_base_id(&session);
    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");
    let owner = common::fixture_player_uid(&session);
    let guild_id = common::fixture_guild_id(&session);
    assert!(
        !blueprint.characters.is_empty(),
        "a full capture of the fixture base must carry pals, or nothing below discriminates"
    );

    let characters_before = character_instance_ids(&session);
    let (handles_before, _, _) = guild_lists(&session, guild_id);
    let dangling_before = works_naming_a_missing_character(&session);
    assert!(
        dangling_before.is_empty(),
        "the fixture must start with no work naming a missing pal: {dangling_before:?}"
    );

    let mut anchor = common::fixture_base_anchor(&session, base_id);
    anchor.x += 20_000.0;
    place::place(
        &mut session,
        &blueprint,
        &merge_request(anchor, base_id, owner),
        &game_data(),
    )
    .expect("merge placement");

    let placed_characters: BTreeSet<Uuid> = character_instance_ids(&session)
        .difference(&characters_before)
        .copied()
        .collect();
    assert_eq!(
        placed_characters.len(),
        blueprint.characters.len(),
        "every pal the blueprint carries must land; a merge that quietly drops them is data loss"
    );

    let named_by_a_work = work_individual_refs(&session)
        .into_iter()
        .filter(|(_, individual)| placed_characters.contains(individual))
        .count();
    assert!(
        named_by_a_work > 0,
        "the merged works must name at least one pal that this merge landed, or the check \
         below can never fail"
    );
    let dangling = works_naming_a_missing_character(&session);
    assert!(
        dangling.is_empty(),
        "every work in the save must name a pal that is in it, got {dangling:?}"
    );

    let (handles_after, _, _) = guild_lists(&session, guild_id);
    assert_eq!(
        handles_after.len() - handles_before.len(),
        placed_characters.len(),
        "the guild must gain one handle per pal that landed"
    );
    let gained: BTreeSet<Uuid> = handles_after[handles_before.len()..]
        .iter()
        .copied()
        .collect();
    assert_eq!(
        gained, placed_characters,
        "the handles the guild gained must be exactly the pals that landed"
    );
}

/// A base names itself in its `WorkCollection.own_id`, and that id is minted by
/// `remap` -- before the placement decides what to call the new base. Reusing it
/// is the only thing that keeps the two in agreement: minting a second id there
/// leaves the placed base's own `WorkCollection` naming a base that exists in no
/// save, and every other placement assertion goes on passing.
#[test]
fn a_placed_bases_work_collection_names_the_base_the_placement_founded() {
    let source = common::load_fixture_session("v1_relics");
    let source_base = common::fixture_base_id(&source);
    let blueprint = source_blueprint(&source, CaptureOptions::full());

    // Read off the source fixture: `own_id` is the base's own id, which is what
    // makes the placed value comparable to the placed base's id at all.
    let source_own_id = work_collection(&source, source_base).own_id;
    assert_eq!(
        source_own_id, source_base,
        "setup: a base's WorkCollection must name the base it belongs to"
    );

    let mut target = common::load_fixture_session("world1");
    let guild_id = first_guild_id(&target);
    let owner = guild_member_uid(&target, guild_id);

    let result = place::place(
        &mut target,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &game_data(),
    )
    .expect("placement");
    let placed_base = result.base_id.expect("a new base must report its id");

    let placed_own_id = work_collection(&target, placed_base).own_id;
    assert_ne!(
        placed_own_id, source_own_id,
        "the placed base must not still name the base it was captured from"
    );
    assert_eq!(
        placed_own_id, placed_base,
        "the placed base's WorkCollection must name the base the placement founded"
    );
}
