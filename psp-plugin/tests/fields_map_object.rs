mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;
use psp_plugin::Access;

const CAPS: &[Capability] = &[Capability::SaveRead, Capability::SaveWrite];

fn first_map_object(body: &str) -> String {
    format!("local target\nfor obj in save.map_objects() do target = obj break end\n{body}")
}

fn error_message(status: RunStatus) -> String {
    match status {
        RunStatus::Error(message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

/// Every row but `hp` and `build_player_uid` is a plain fact about the
/// save's record or the instance's identity that nothing here writes. A row
/// given a write later has to justify itself here rather than arrive
/// quietly.
#[test]
fn only_hp_and_build_player_uid_are_writable() {
    for spec in psp_plugin::MAP_OBJECT_FIELDS {
        if spec.name == "hp" || spec.name == "build_player_uid" {
            assert_eq!(spec.access, Access::ReadWrite, "{} must be a writable row", spec.name);
        } else {
            assert_eq!(
                spec.access,
                Access::ReadOnly,
                "{} claims to be writable; this handle has no write path for it",
                spec.name
            );
        }
    }
}

#[test]
fn map_object_hp_is_assignable_and_max_hp_is_not() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local repaired = 0
         for obj in save.map_objects() do
           if obj.hp < obj.max_hp then
             obj.hp = obj.max_hp
             repaired = repaired + 1
           end
         end
         local still_damaged = 0
         for obj in save.map_objects() do
           if obj.hp < obj.max_hp then still_damaged = still_damaged + 1 end
         end
         return tostring(repaired) .. ',' .. tostring(still_damaged)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("467,0"),
        "467 is the fixture's damaged structure count, independently confirmed in psp-core"
    );
}

#[test]
fn assigning_max_hp_is_refused_rather_than_silently_ignored() {
    let mut harness = support::harness(CAPS);
    let message =
        error_message(harness.run(&first_map_object("target.max_hp = 1\nreturn 'unreachable'")).0);
    assert!(message.contains("max_hp"), "the refusal must name the field, got {message:?}");
    assert!(message.contains("read-only"), "got {message:?}");
}

/// `hp` accepts any `i32`, including a value below `max_hp` -- a plugin that
/// damages a structure is doing something legitimate, and a silent clamp would
/// make the assignment look like it did something other than what it asked for.
#[test]
fn assigning_hp_is_not_clamped_to_max_hp() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local target
         for obj in save.map_objects() do
           if obj.max_hp > 0 then target = obj break end
         end
         target.hp = 0
         return tostring(target.hp)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("0"), "a write below max_hp must not be clamped back up");
}

#[test]
fn assigning_build_player_uid_nil_clears_it_and_a_uuid_sets_it() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local target
         for obj in save.map_objects() do
           if obj.build_player_uid ~= nil then target = obj break end
         end
         local before = target.build_player_uid
         target.build_player_uid = nil
         local after_clear = target.build_player_uid
         target.build_player_uid = before
         local after_reset = target.build_player_uid
         return tostring(before ~= nil) .. ',' .. tostring(after_clear) .. ',' .. tostring(after_reset == before)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("true,nil,true"),
        "the fixture must carry a built structure, or this test is vacuous"
    );
}

#[test]
fn assigning_build_player_uid_an_invalid_string_is_refused() {
    let mut harness = support::harness(CAPS);
    let message = error_message(
        harness.run(&first_map_object("target.build_player_uid = 'not-a-uuid'\nreturn 'unreachable'")).0,
    );
    assert!(message.contains("build_player_uid"), "must name the field, got {message:?}");
}

#[test]
fn assigning_the_identity_field_raises() {
    let mut harness = support::harness(CAPS);
    let message =
        error_message(harness.run(&first_map_object("target.instance_id = 'x'\nreturn 'unreachable'")).0);
    assert!(message.contains("instance_id"), "must name the field, got {message:?}");
    assert!(message.contains("read-only"), "must say it is read-only, got {message:?}");
}

#[test]
fn an_unknown_field_raises_rather_than_silently_succeeding() {
    let mut harness = support::harness(CAPS);
    let message =
        error_message(harness.run(&first_map_object("target.hpp = 4\nreturn 'unreachable'")).0);
    assert!(message.contains("hpp"), "must name the field, got {message:?}");
}

#[test]
fn assignment_without_save_write_raises() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let message =
        error_message(harness.run(&first_map_object("target.hp = 1\nreturn 'unreachable'")).0);
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must say which capability is missing, got {message:?}"
    );
    for wrong in ["unknown map_object field", "read-only", "attempt to index"] {
        assert!(
            !message.contains(wrong),
            "the refusal must be the capability one, not {wrong:?}, got {message:?}"
        );
    }
}

/// The gate runs before the field name is resolved, so an ungranted plugin is
/// never told whether a field exists.
#[test]
fn an_ungranted_assignment_is_refused_before_the_field_name_is_resolved() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let message = error_message(
        harness.run(&first_map_object("target.no_such_field = 4\nreturn 'unreachable'")).0,
    );
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must be refused for the capability, got {message:?}"
    );
    assert!(
        !message.contains("unknown map_object field"),
        "refusing an ungranted plugin by field name tells it which fields exist, got {message:?}"
    );
}

/// Reading is untouched by the write surface this task added, and stays on
/// `save.read` alone.
#[test]
fn every_row_still_reads_with_save_read_alone() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let (status, summary) = harness.run(&first_map_object(
        "return tostring(target.id) .. '|' .. tostring(target.instance_id) .. '|' .. tostring(target.kind)",
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let mut parts = summary.split('|');
    let id = parts.next().expect("id");
    let instance_id = parts.next().expect("instance_id");
    let kind = parts.next().expect("kind");
    assert!(!id.is_empty() && id != "nil", "id must read as a non-empty string, got {id:?}");
    assert!(
        uuid::Uuid::parse_str(instance_id).is_ok(),
        "instance_id must read as a uuid, got {instance_id:?}"
    );
    assert!(!kind.is_empty() && kind != "nil", "kind must read as a non-empty string, got {kind:?}");
}
