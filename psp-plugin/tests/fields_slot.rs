mod support;

use psp_core::dto::container::ItemContainerDto;
use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;
use uuid::Uuid;

const CAPS: &[Capability] = &[Capability::SaveRead, Capability::SaveWrite];

/// Reading a pal field rebuilds the pal snapshot, and that is what flushes the
/// DTO cache out to the save. A slot write does not go through that cache at
/// all, which is exactly what the tests either side of it are checking.
const FORCE_FLUSH: &str = "for p in save.pals() do local _ = p.level break end\n";

fn error_message(status: RunStatus) -> String {
    match status {
        RunStatus::Error(message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

/// Loaded once per test binary: `GameData::load` reads the whole `data/json`
/// tree, and the read-back helpers below want it on every call.
fn game_data() -> &'static psp_core::gamedata::GameData {
    static GAME_DATA: std::sync::OnceLock<psp_core::gamedata::GameData> =
        std::sync::OnceLock::new();
    GAME_DATA.get_or_init(|| {
        let data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("psp-plugin has a parent directory")
            .join("data/json");
        psp_core::gamedata::GameData::load(&data_dir).expect("game data is checked in")
    })
}

/// The container as `psp-core` reads it back off the save, with no part of the
/// plugin host between: not through a handle, not through the run's cached
/// copy. If the write never reached the save, this reports the old values.
fn container_in_the_save(harness: &mut support::Harness, id: Uuid) -> ItemContainerDto {
    let game_data = game_data();
    let session = harness.session_mut();
    let level = &session.level;
    let caches = &mut session.caches;
    psp_core::domain::containers::read_item_container(level, caches, game_data, id, "", None)
        .expect("the container must still be readable")
}

fn slot_in_the_save(
    harness: &mut support::Harness,
    container_id: Uuid,
    slot_index: i32,
) -> Option<psp_core::dto::container::ItemContainerSlotDto> {
    container_in_the_save(harness, container_id)
        .slots
        .into_iter()
        .find(|slot| slot.slot_index == slot_index)
}

/// Whether the save still holds a `DynamicItemSaveData` entry at this local id.
/// Independent of `read_item_container`: a slot whose record was deleted keeps
/// pointing at it, and the reader drops such a slot entirely rather than
/// reporting the dangling reference.
fn dynamic_item_entry_exists(harness: &support::Harness, local_id: Uuid) -> bool {
    psp_core::domain::world::build_dynamic_item_index(&harness.session().level)
        .contains_key(&local_id)
}

fn container_ids(harness: &support::Harness) -> Vec<Uuid> {
    harness
        .session()
        .item_container_map()
        .expect("the corpus fixture has item containers")
        .iter()
        .filter_map(|entry| {
            psp_core::props::get(psp_core::props::struct_props(&entry.key)?, &["ID"])
                .and_then(psp_core::props::as_uuid)
        })
        .collect()
}

/// A fixture slot that genuinely carries a per-item record with values a test
/// can watch: a weapon, whose durability and remaining rounds are read straight
/// off the record. A dynamic-item test run against a slot with nothing to lose
/// passes and proves nothing, so this refuses to hand one back.
fn a_weapon_slot(harness: &mut support::Harness) -> (Uuid, i32) {
    static FOUND: std::sync::OnceLock<Option<(Uuid, i32)>> = std::sync::OnceLock::new();
    let found = scan_slots(harness, &FOUND, |slot| {
        slot.dynamic_item
            .as_ref()
            .is_some_and(|record| record.durability.unwrap_or(0.0) > 0.0 && record.remaining_bullets.is_some())
    });
    found.expect("the corpus fixture must hold a slot carrying a weapon record")
}

/// The first slot in the save the predicate accepts, in the order
/// `save.containers()` walks them.
///
/// Cached per binary, not per call: every harness loads the same corpus from
/// disk, so the answer cannot differ between them, and the scan walks all 4873
/// containers. `cache` is the caller's own `OnceLock`, so each predicate keeps
/// its own answer. A test that has already written to its slot still gets the
/// same slot back, which is what its own fresh harness holds.
fn scan_slots(
    harness: &mut support::Harness,
    cache: &'static std::sync::OnceLock<Option<(Uuid, i32)>>,
    accept: impl Fn(&psp_core::dto::container::ItemContainerSlotDto) -> bool,
) -> Option<(Uuid, i32)> {
    if let Some(found) = cache.get() {
        return *found;
    }
    let game_data = game_data();
    let mut found = None;
    for id in container_ids(harness) {
        let session = harness.session_mut();
        let level = &session.level;
        let caches = &mut session.caches;
        let Some(dto) = psp_core::domain::containers::read_item_container(
            level, caches, game_data, id, "", None,
        ) else {
            continue;
        };
        if let Some(slot) = dto.slots.iter().find(|slot| accept(slot)) {
            found = Some((id, slot.slot_index));
            break;
        }
    }
    *cache.get_or_init(|| found)
}

/// A fixture slot carrying no per-item record, so `item_id` can be assigned on
/// it at all.
fn a_plain_slot(harness: &mut support::Harness) -> (Uuid, i32) {
    static FOUND: std::sync::OnceLock<Option<(Uuid, i32)>> = std::sync::OnceLock::new();
    let found = scan_slots(harness, &FOUND, |slot| {
        slot.dynamic_item.is_none()
            && !matches!(slot.static_id.as_deref(), Some("") | Some("None") | None)
    });
    found.expect("the corpus fixture must hold an occupied slot with no per-item record")
}

/// Walks to one named slot through the public surface an author would use.
fn slot_script(container_id: Uuid, slot_index: i32, body: &str) -> String {
    format!(
        "local target\n\
         for c in save.containers() do\n\
         \x20 if tostring(c.id) == '{container_id}' then\n\
         \x20   for s in c.slots() do if s.index == {slot_index} then target = s break end end\n\
         \x20   break\n\
         \x20 end\n\
         end\n\
         assert(target ~= nil, 'the fixture slot must be reachable')\n\
         {body}"
    )
}

/// THE one. `apply_item_container_dto` deletes the record a slot already had
/// whenever the incoming slot carries none, so the obvious way to write a count
/// destroys the item's durability, rounds, skills and talents without a word.
/// Assign `count`, then read the record back from the save and prove every part
/// of it survived -- including, independently of the container reader, that the
/// `DynamicItemSaveData` entry itself is still there.
#[test]
fn a_count_write_preserves_the_slots_per_item_record() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_weapon_slot(&mut harness);

    let before = slot_in_the_save(&mut harness, container_id, slot_index)
        .expect("the fixture slot must exist before the write");
    let record_before = before.dynamic_item.clone().expect("the fixture slot must carry a record");
    assert!(
        record_before.durability.unwrap_or(0.0) > 0.0,
        "the fixture slot must have durability to lose, got {:?}",
        record_before.durability
    );
    assert!(
        dynamic_item_entry_exists(&harness, record_before.local_id),
        "the record must be in the save before the write"
    );
    assert_ne!(before.count, 5, "the fixture slot must not already hold the count this writes");

    let (status, summary) =
        harness.run(&slot_script(container_id, slot_index, "target.count = 5\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok, "the write must be accepted: {summary:?}");

    let after = slot_in_the_save(&mut harness, container_id, slot_index)
        .expect("the slot must still exist after the write");
    assert_eq!(after.count, 5, "the count must have reached the save");
    assert_eq!(after.static_id, before.static_id, "the item must not have changed");

    let record_after = after.dynamic_item.expect("the per-item record must have survived the write");
    assert_eq!(record_after.local_id, record_before.local_id);
    assert_eq!(record_after.r#type, record_before.r#type);
    assert_eq!(record_after.durability, record_before.durability);
    assert_eq!(record_after.remaining_bullets, record_before.remaining_bullets);
    assert_eq!(record_after.passive_skill_list, record_before.passive_skill_list);
    assert_eq!(record_after.static_id, record_before.static_id);
    assert!(
        dynamic_item_entry_exists(&harness, record_before.local_id),
        "the DynamicItemSaveData entry must still be in the save"
    );
}

/// The same hazard measured from the other end, and blind to where the damage
/// would be: every slot in the container, before and after, compared whole. A
/// write that carried the whole container into the apply path would empty every
/// slot the save spells `"None"` and strip every record along the way.
#[test]
fn a_slot_write_leaves_every_other_slot_in_its_container_untouched() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_weapon_slot(&mut harness);

    let census = |dto: &ItemContainerDto| -> Vec<String> {
        dto.slots
            .iter()
            .map(|slot| {
                format!(
                    "{}={:?}x{} record={:?}",
                    slot.slot_index,
                    slot.static_id,
                    slot.count,
                    slot.dynamic_item.as_ref().map(|record| (
                        record.local_id,
                        record.r#type.clone(),
                        record.durability,
                        record.remaining_bullets
                    ))
                )
            })
            .collect()
    };

    let others = |rows: &[String]| -> Vec<String> {
        let written = format!("{slot_index}=");
        rows.iter().filter(|row| !row.starts_with(&written)).cloned().collect()
    };

    let before = census(&container_in_the_save(&mut harness, container_id));
    assert!(
        !others(&before).is_empty(),
        "the fixture container must hold slots other than the one being written"
    );

    let (status, _) =
        harness.run(&slot_script(container_id, slot_index, "target.count = 7\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);

    let after = census(&container_in_the_save(&mut harness, container_id));
    assert_eq!(before.len(), after.len(), "the container must not have gained or lost a slot");
    assert_eq!(others(&before), others(&after), "a slot other than the one written changed");
}

/// The value-dependent half of `item_id`. `"None"` routes to the same entry
/// removal `slot.clear()` performs, so it is refused rather than performed
/// under the guise of a field write -- and the refusal names the operation that
/// does work. The empty string reads back as nil the same way `"None"` does,
/// and nil is what an author reaches for when they mean "empty this".
#[test]
fn assigning_an_empty_item_id_raises_and_names_slot_clear() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    for value in ["'None'", "''", "nil"] {
        let message = error_message(
            harness
                .run(&slot_script(
                    container_id,
                    slot_index,
                    &format!("target.item_id = {value}\nreturn 'unreachable'"),
                ))
                .0,
        );
        assert!(message.contains("item_id"), "must name the field, got {message:?}");
        assert!(
            message.contains("slot.clear()"),
            "must point at the operation that empties a slot, got {message:?}"
        );
    }
}

/// And the value that is refused must genuinely still be in the save
/// afterwards: a refusal that had already removed the entry would read exactly
/// like a refusal that had not.
#[test]
fn a_refused_empty_item_id_leaves_the_slot_in_the_save() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let before = slot_in_the_save(&mut harness, container_id, slot_index).expect("the slot exists");

    let (status, _) = harness.run(&slot_script(
        container_id,
        slot_index,
        "pcall(function() target.item_id = 'None' end)\nreturn 'ok'",
    ));
    assert_eq!(status, RunStatus::Ok);

    let after = slot_in_the_save(&mut harness, container_id, slot_index)
        .expect("the refused assignment must not have removed the slot");
    assert_eq!(after.static_id, before.static_id);
    assert_eq!(after.count, before.count);
}

/// A record names its own item and nothing here can rewrite it, so re-pointing
/// only the slot would leave the two disagreeing.
#[test]
fn assigning_item_id_on_a_slot_carrying_a_record_raises() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_weapon_slot(&mut harness);
    let message = error_message(
        harness
            .run(&slot_script(container_id, slot_index, "target.item_id = 'Wood'\nreturn 'unreachable'"))
            .0,
    );
    assert!(message.contains("item_id"), "must name the field, got {message:?}");
    assert!(
        message.contains("per-item record"),
        "must say what stands in the way, got {message:?}"
    );
}

/// The one thing a green field-assignment suite can still fail to say: that the
/// write reached the save at all.
#[test]
fn a_written_item_id_and_count_reach_the_save() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let before = slot_in_the_save(&mut harness, container_id, slot_index).expect("the slot exists");
    assert_ne!(before.static_id.as_deref(), Some("Wood"), "the fixture slot must not already hold Wood");

    let (status, _) = harness.run(&slot_script(
        container_id,
        slot_index,
        "target.item_id = 'Wood'\ntarget.count = 42\nreturn 'ok'",
    ));
    assert_eq!(status, RunStatus::Ok);

    let after = slot_in_the_save(&mut harness, container_id, slot_index)
        .expect("the slot must still exist");
    assert_eq!(after.static_id.as_deref(), Some("Wood"), "the item id must reach the raw slot");
    assert_eq!(after.count, 42, "the count must reach the raw slot");
}

/// The handle must report what the save holds, at every point in the run. A
/// slot write lands immediately rather than waiting for a flush, so the read
/// after it is a read of the save -- and the read after a flush has to agree
/// with it.
#[test]
fn a_written_count_reads_the_same_on_both_sides_of_a_flush() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_weapon_slot(&mut harness);
    let (status, summary) = harness.run(&slot_script(
        container_id,
        slot_index,
        &format!(
            "local original = tostring(target.count)\n\
             target.count = 9\n\
             local written = tostring(target.count)\n\
             {FORCE_FLUSH}\
             local flushed = tostring(target.count)\n\
             return original .. '|' .. written .. '|' .. flushed"
        ),
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let mut parts = summary.split('|');
    let original = parts.next().expect("the original count");
    let written = parts.next().expect("the written count");
    let flushed = parts.next().expect("the count after a flush");
    assert_ne!(original, "9", "the fixture slot must not already hold this count");
    assert_eq!(written, "9");
    assert_eq!(flushed, written, "the count must read the same on both sides of a flush");
}

/// A slot value overwritten in place adds and removes no entry, so nothing a
/// handle or iterator points at has moved.
#[test]
fn a_slot_write_leaves_every_live_handle_and_iterator_valid() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_weapon_slot(&mut harness);
    let (status, summary) = harness.run(&slot_script(
        container_id,
        slot_index,
        "local guild, pal, container\n\
         for g in save.guilds() do guild = g break end\n\
         for p in save.pals() do pal = p break end\n\
         for c in save.containers() do container = c break end\n\
         local containers = save.containers()\n\
         local first = containers()\n\
         target.count = 3\n\
         local out = {}\n\
         out[#out+1] = tostring(target.count)\n\
         out[#out+1] = tostring(guild.id ~= nil)\n\
         out[#out+1] = tostring(pal.instance_id ~= nil)\n\
         out[#out+1] = tostring(container.slot_count ~= nil)\n\
         out[#out+1] = tostring(first.id ~= nil)\n\
         out[#out+1] = tostring(containers() ~= nil)\n\
         return table.concat(out, ',')",
    ));
    assert_eq!(status, RunStatus::Ok, "every handle and the part-consumed iterator must survive: {summary:?}");
    assert_eq!(summary.as_deref(), Some("3,true,true,true,true,true"));
}

#[test]
fn assigning_the_index_raises() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let message = error_message(
        harness.run(&slot_script(container_id, slot_index, "target.index = 0\nreturn 'unreachable'")).0,
    );
    assert!(message.contains("index"), "must name the field, got {message:?}");
    assert!(message.contains("read-only"), "must say it is read-only, got {message:?}");
}

#[test]
fn a_count_below_one_raises_and_names_slot_clear() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    for value in ["0", "-3"] {
        let message = error_message(
            harness
                .run(&slot_script(
                    container_id,
                    slot_index,
                    &format!("target.count = {value}\nreturn 'unreachable'"),
                ))
                .0,
        );
        assert!(message.contains("count"), "must name the field, got {message:?}");
        assert!(message.contains(value), "must name the value, got {message:?}");
        assert!(
            message.contains("slot.clear()"),
            "must point at the operation that empties a slot, got {message:?}"
        );
    }
}

/// The save holds the count as an `i32`. A wider value would arrive narrowed
/// rather than as the number that was assigned.
#[test]
fn a_count_wider_than_the_save_raises() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let message = error_message(
        harness
            .run(&slot_script(container_id, slot_index, "target.count = 5000000000\nreturn 'unreachable'"))
            .0,
    );
    assert!(message.contains("count"), "must name the field, got {message:?}");
    assert!(message.contains("5000000000"), "must name the value, got {message:?}");
}

#[test]
fn an_unknown_field_raises_rather_than_silently_succeeding() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let message = error_message(
        harness.run(&slot_script(container_id, slot_index, "target.cuont = 1\nreturn 'unreachable'")).0,
    );
    assert!(message.contains("cuont"), "must name the field, got {message:?}");
}

#[test]
fn a_wrong_typed_value_raises() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let message = error_message(
        harness
            .run(&slot_script(container_id, slot_index, "target.count = 'many'\nreturn 'unreachable'"))
            .0,
    );
    assert!(message.contains("count"), "must name the field, got {message:?}");
    assert!(message.contains("string"), "must name the type it got, got {message:?}");
}

/// `count` is read-write and 4 is a legal count, so the only thing left that can
/// refuse this is the capability gate.
#[test]
fn assignment_without_save_write_raises() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let message = error_message(
        harness.run(&slot_script(container_id, slot_index, "target.count = 4\nreturn 'unreachable'")).0,
    );
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must say which capability is missing, got {message:?}"
    );
    for wrong in ["unknown slot field", "is read-only", "attempt to index", "at least 1"] {
        assert!(
            !message.contains(wrong),
            "the refusal must be the capability one, not {wrong:?}, got {message:?}"
        );
    }
}

/// The gate runs before the field name is resolved. With a real field name the
/// gate fires either way and the message is identical, so only an unknown name
/// can detect the reordering.
#[test]
fn an_ungranted_assignment_is_refused_before_the_field_name_is_resolved() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let message = error_message(
        harness
            .run(&slot_script(container_id, slot_index, "target.no_such_field = 4\nreturn 'unreachable'"))
            .0,
    );
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must be refused for the capability, got {message:?}"
    );
    assert!(
        !message.contains("unknown slot field"),
        "refusing an ungranted plugin by field name tells it which fields exist, got {message:?}"
    );
}

#[test]
fn a_dry_run_reads_back_what_it_just_set() {
    let mut harness = support::harness_dry(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let (status, summary) = harness.run(&slot_script(
        container_id,
        slot_index,
        "target.count = 11\ntarget.item_id = 'Wood'\n\
         return tostring(target.count) .. ',' .. tostring(target.item_id)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("11,Wood"));
}

/// A real run's flush drains the DTO cache and the read afterwards comes off the
/// save; a dry run's must keep previewing its own accepted assignment, or the
/// write it is previewing vanishes from the rest of its own preview.
#[test]
fn a_dry_run_still_reads_back_its_own_write_across_a_mid_run_flush() {
    let mut harness = support::harness_dry(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let (status, summary) = harness.run(&slot_script(
        container_id,
        slot_index,
        &format!(
            "target.count = 13\n\
             local before = tostring(target.count)\n\
             {FORCE_FLUSH}\
             local after = tostring(target.count)\n\
             return before .. '|' .. after"
        ),
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("13|13"),
        "a dry run must keep reading its own accepted write after a flush it never performed"
    );
}

/// And the preview must survive the run's cached copy of the container being
/// dropped, which reading any other container does.
#[test]
fn a_dry_run_still_reads_back_its_own_write_after_another_container_is_read() {
    let mut harness = support::harness_dry(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let (status, summary) = harness.run(&slot_script(
        container_id,
        slot_index,
        "target.count = 17\n\
         local other = 0\n\
         for c in save.containers() do other = other + (c.slot_count or 0) end\n\
         return tostring(target.count)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("17"));
}

/// A dry run must reach the save with nothing at all, however convincingly it
/// reads back inside the run.
#[test]
fn a_dry_run_leaves_the_slot_in_the_save_untouched() {
    let mut harness = support::harness_dry(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let before = slot_in_the_save(&mut harness, container_id, slot_index).expect("the slot exists");
    let (status, _) = harness.run(&slot_script(
        container_id,
        slot_index,
        "target.count = 23\ntarget.item_id = 'Wood'\nreturn 'ok'",
    ));
    assert_eq!(status, RunStatus::Ok);
    let after = slot_in_the_save(&mut harness, container_id, slot_index).expect("the slot exists");
    assert_eq!(after.count, before.count);
    assert_eq!(after.static_id, before.static_id);
}

#[test]
fn a_dry_run_counts_each_accepted_assignment_and_a_real_run_counts_none() {
    let mut dry = support::harness_dry(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut dry);
    let (status, _) = dry.run(&slot_script(
        container_id,
        slot_index,
        "target.count = 2\ntarget.count = 3\ntarget.item_id = 'Wood'\nreturn 'ok'",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(dry.counts().get("slot.count").copied(), Some(2));
    assert_eq!(dry.counts().get("slot.item_id").copied(), Some(1));

    let mut real = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut real);
    let (status, _) =
        real.run(&slot_script(container_id, slot_index, "target.count = 2\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(real.counts().get("slot.count"), None);
}

#[test]
fn a_dry_run_does_not_count_a_refused_assignment() {
    let mut harness = support::harness_dry(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let (status, _) = harness.run(&slot_script(
        container_id,
        slot_index,
        "pcall(function() target.count = 0 end)\n\
         pcall(function() target.item_id = 'None' end)\nreturn 'ok'",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(harness.counts().get("slot.count"), None);
    assert_eq!(harness.counts().get("slot.item_id"), None);
}

/// `slot.clear()` is the structural neighbour of these assignments and has to
/// stay structural: it removes the slot entry, so everything live must be
/// refused afterwards.
#[test]
fn clear_still_bumps_the_epoch_and_invalidates_the_handle_that_called_it() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let (status, summary) = harness.run(&slot_script(
        container_id,
        slot_index,
        "target.clear()\nreturn tostring(pcall(function() return target.count end))",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("false"));
}

/// An id the loaded game data does not hold would be written into the raw slot
/// verbatim and name an item the game cannot resolve. The catalog is on
/// `GameData`, which the host holds unconditionally, so the check costs the
/// plugin no capability it did not already need to reach the slot.
#[test]
fn an_unknown_item_id_raises_and_names_the_catalog() {
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let before = slot_in_the_save(&mut harness, container_id, slot_index).expect("the slot exists");

    let message = error_message(
        harness
            .run(&slot_script(
                container_id,
                slot_index,
                "target.item_id = 'NotARealItemId'\nreturn 'unreachable'",
            ))
            .0,
    );
    assert!(message.contains("item_id"), "must name the field, got {message:?}");
    assert!(message.contains("NotARealItemId"), "must name the value, got {message:?}");
    assert!(message.contains("items catalog"), "must say what refused it, got {message:?}");

    let after = slot_in_the_save(&mut harness, container_id, slot_index)
        .expect("the refused assignment must leave the slot alone");
    assert_eq!(after.static_id, before.static_id);
}

/// Save ids and `items.json` do not agree on casing, so the match is
/// case-insensitive -- and the id reaches the raw slot exactly as written,
/// because nothing rewrites it to the catalog's spelling. Both halves matter:
/// an exact-match check would refuse ids the save already contains.
#[test]
fn a_known_item_id_is_accepted_whatever_its_casing_and_stored_as_written() {
    assert!(
        !game_data().is_known_item_key("NotARealItemId"),
        "the negative test above must be testing an id the catalog genuinely lacks"
    );
    let mut harness = support::harness(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);

    let (status, _) = harness.run(&slot_script(
        container_id,
        slot_index,
        "target.item_id = 'wOoD'\nreturn 'ok'",
    ));
    assert_eq!(status, RunStatus::Ok, "a differently-cased known id must be accepted");
    assert_eq!(
        slot_in_the_save(&mut harness, container_id, slot_index)
            .expect("the slot exists")
            .static_id
            .as_deref(),
        Some("wOoD"),
        "the id must reach the save exactly as written"
    );
}

/// An unavailable catalog is not evidence that an id is wrong. `GameData::load`
/// leaves the catalog empty rather than erroring when `items.json` is missing or
/// malformed, and `is_known_item_key` answers false for everything against an
/// empty set -- so a missing file must not become a wall across the whole row.
#[test]
fn an_unavailable_catalog_does_not_refuse_every_write() {
    let mut harness = support::harness(CAPS).with_empty_game_data();
    let (container_id, slot_index) = a_plain_slot(&mut harness);

    let (status, _) = harness.run(&slot_script(
        container_id,
        slot_index,
        "target.item_id = 'NotARealItemId'\nreturn 'ok'",
    ));
    assert_eq!(status, RunStatus::Ok, "an empty catalog must turn the check off, not on");
    assert_eq!(
        slot_in_the_save(&mut harness, container_id, slot_index)
            .expect("the slot exists")
            .static_id
            .as_deref(),
        Some("NotARealItemId")
    );
}

/// A dry run cannot invalidate handles -- no structural operation calls
/// `note_mutation` under one -- so a script can read a slot it has just
/// previewed clearing. What it must not read is a pending count for an entry the
/// same preview said would be gone: the clear drops the record and the read
/// falls back to what the save holds.
#[test]
fn a_dry_run_clear_drops_the_pending_value_for_that_slot() {
    let mut harness = support::harness_dry(CAPS);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let (status, summary) = harness.run(&slot_script(
        container_id,
        slot_index,
        "local original = tostring(target.count)\n\
         target.count = 31\n\
         local pending = tostring(target.count)\n\
         target.clear()\n\
         local cleared = tostring(target.count)\n\
         return original .. '|' .. pending .. '|' .. cleared",
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let mut parts = summary.split('|');
    let original = parts.next().expect("the original count");
    let pending = parts.next().expect("the previewed count");
    let cleared = parts.next().expect("the count after the previewed clear");
    assert_ne!(original, "31", "the fixture slot must not already hold this count");
    assert_eq!(pending, "31", "the preview must read back its own assignment");
    assert_eq!(
        cleared, original,
        "a previewed clear must drop the previewed count rather than keep reporting it"
    );
}
