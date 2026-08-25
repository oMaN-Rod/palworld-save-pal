mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;
use psp_plugin::Access;

const CAPS: &[Capability] = &[Capability::SaveRead, Capability::SaveWrite];

fn first_container(body: &str) -> String {
    format!("local target\nfor c in save.containers() do target = c break end\n{body}")
}

fn error_message(status: RunStatus) -> String {
    match status {
        RunStatus::Error(message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

/// Nothing on this handle can be assigned, and that is the finished state
/// rather than a gap: `id` is identity, `slots` is an iterator rather than a
/// field, and `slot_count` is structural. A row given a write later has to
/// justify itself here rather than arrive quietly.
#[test]
fn no_container_row_is_writable() {
    for spec in psp_plugin::CONTAINER_FIELDS {
        assert_eq!(
            spec.access,
            Access::ReadOnly,
            "{} claims to be writable; this handle has no write path",
            spec.name
        );
    }
}

/// The refusal has to point at the operation that works. An author who reaches
/// for the assignment wants the container resized, and there is a call that
/// does it.
#[test]
fn slot_count_is_not_assignable() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(
        "for c in save.containers() do c.slot_count = 20 break end
         return 'unreachable'",
    );
    match status {
        RunStatus::Error(message) => assert!(
            message.contains("set_slot_count"),
            "the refusal must point at the function to use instead, got {message:?}"
        ),
        other => panic!("expected an error, got {other:?}"),
    }
}

/// The one that matters. A structural operation must stay structural: this task
/// added a `__newindex` to the container metatable, and the thing that could
/// have gone wrong is `set_slot_count` quietly becoming a field write that
/// leaves handles valid over a container whose slots moved.
#[test]
fn set_slot_count_still_bumps_the_epoch_and_invalidates_iterators() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local first
         for c in save.containers() do first = c break end
         first.set_slot_count(first.slot_count + 1)
         local ok = pcall(function() return first.slot_count end)
         return tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("false"),
        "a structural write must still invalidate the handle that performed it"
    );
}

/// The other half: an iterator opened before the resize must be refused too,
/// not merely the handle that performed it.
#[test]
fn set_slot_count_still_invalidates_an_iterator_opened_before_it() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local first
         for c in save.containers() do first = c break end
         local containers = save.containers()
         assert(containers() ~= nil, 'the iterator must yield before the resize')
         first.set_slot_count(first.slot_count + 1)
         local ok = pcall(containers)
         return tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("false"));
}

#[test]
fn assigning_the_identity_field_raises() {
    let mut harness = support::harness(CAPS);
    let message =
        error_message(harness.run(&first_container("target.id = 'x'\nreturn 'unreachable'")).0);
    assert!(message.contains("id"), "must name the field, got {message:?}");
    assert!(message.contains("read-only"), "must say it is read-only, got {message:?}");
    assert!(
        !message.contains("set_slot_count"),
        "identity is not structural and must not be sent to the resize call, got {message:?}"
    );
}

#[test]
fn an_unknown_field_raises_rather_than_silently_succeeding() {
    let mut harness = support::harness(CAPS);
    let message =
        error_message(harness.run(&first_container("target.slto_count = 4\nreturn 'unreachable'")).0);
    assert!(message.contains("slto_count"), "must name the field, got {message:?}");
}

/// `slot_count` is a real field and 20 is a legal count, so the only thing left
/// that can refuse this is the capability gate. The negatives are what stop the
/// test passing for a different reason if that ever stops being true.
#[test]
fn assignment_without_save_write_raises() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let message = error_message(
        harness.run(&first_container("target.slot_count = 20\nreturn 'unreachable'")).0,
    );
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must say which capability is missing, got {message:?}"
    );
    for wrong in ["unknown container field", "set_slot_count", "read-only", "attempt to index"] {
        assert!(
            !message.contains(wrong),
            "the refusal must be the capability one, not {wrong:?}, got {message:?}"
        );
    }
}

/// The gate runs before the field name is resolved, so an ungranted plugin is
/// never told whether a field exists. With a real field name the gate fires
/// either way and the message is identical, so only an unknown name can detect
/// the reordering.
#[test]
fn an_ungranted_assignment_is_refused_before_the_field_name_is_resolved() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let message = error_message(
        harness.run(&first_container("target.no_such_field = 4\nreturn 'unreachable'")).0,
    );
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must be refused for the capability, got {message:?}"
    );
    assert!(
        !message.contains("unknown container field"),
        "refusing an ungranted plugin by field name tells it which fields exist, got {message:?}"
    );
}

/// Reading is untouched by the write surface this task added, and stays on
/// `save.read` alone.
#[test]
fn every_row_still_reads_with_save_read_alone() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let (status, summary) = harness
        .run(&first_container("return tostring(target.id) .. '|' .. tostring(target.slot_count)"));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let (id, slot_count) = summary.split_once('|').expect("id|slot_count");
    assert!(uuid::Uuid::parse_str(id).is_ok(), "id must read as a uuid, got {id:?}");
    assert!(slot_count.parse::<i64>().is_ok(), "slot_count must read as an integer, got {slot_count:?}");
}
