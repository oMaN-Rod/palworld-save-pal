mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

const CAPS: &[Capability] = &[Capability::SaveRead, Capability::SaveWrite];

fn first_base(body: &str) -> String {
    format!("local target\nfor b in save.bases() do target = b break end\n{body}")
}

fn error_message(status: RunStatus) -> String {
    match status {
        RunStatus::Error(message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

/// Reading a pal field rebuilds the pal snapshot, and that is what flushes the
/// DTO cache out to the save -- which also drains the base entry, so the next
/// read of a base field comes back out of the `BaseCampSaveData` entry rather
/// than the cache.
const FORCE_FLUSH: &str = "for p in save.pals() do local _ = p.level break end\n";

fn a_base_id(harness: &support::Harness) -> uuid::Uuid {
    let entries = harness.session().base_camp_map().expect("the corpus fixture has bases");
    let entry = entries.first().expect("the corpus fixture has bases");
    psp_core::props::as_uuid(&entry.key).expect("a base entry is keyed by its uuid")
}

/// The base as `psp-core` reads it back off the save, with no part of the
/// plugin host between: the base's owning guild is loaded whole and the base
/// picked out of it. Nothing about how this task caches or counts a base write
/// is visible here, so if `apply_base_dto` did not actually happen this
/// reports the old values.
fn base_in_the_save(
    harness: &mut support::Harness,
    id: uuid::Uuid,
) -> psp_core::dto::guild::BaseDto {
    let data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("psp-plugin has a parent directory")
        .join("data/json");
    let game_data = psp_core::gamedata::GameData::load(&data_dir).expect("game data is checked in");
    let guild_ids: Vec<uuid::Uuid> = harness.session().guild_summary_order.clone();
    for guild_id in guild_ids {
        let Some(dto) =
            psp_core::domain::guild::get_guild_details(harness.session_mut(), &game_data, guild_id)
                .expect("a fixture guild must load")
        else {
            continue;
        };
        if let Some(base) = dto.bases.and_then(|bases| bases.get(&id).cloned()) {
            return base;
        }
    }
    panic!("base {id} is in no guild's base list");
}

/// Strips the `RawData` off the save's last `BaseCampSaveData` entry, leaving
/// an entry that is still keyed by a uuid and so still handed out by
/// `save.bases()`, but whose base-camp record cannot be read. That is the
/// state `psp-core`'s own `build_guild_dto` answers `(None, None, None)` for,
/// and the state this handle already answered nil for on `x`/`y`/`z`.
///
/// Returns the id, which `save.bases()` will yield last.
fn break_the_last_bases_record(harness: &mut support::Harness) -> uuid::Uuid {
    let entries = psp_core::domain::world::base_camp_map_mut(&mut harness.session_mut().level)
        .expect("the corpus fixture has a base camp map")
        .expect("the corpus fixture has a base camp map");
    let entry = entries.last_mut().expect("the corpus fixture has bases");
    let id = psp_core::props::as_uuid(&entry.key).expect("a base entry is keyed by its uuid");
    psp_core::props::struct_props_mut(&mut entry.value)
        .expect("a base entry's value is a struct")
        .0
        .shift_remove(&psp_core::ue::PropertyKey::from("RawData"));
    id
}

fn last_base(body: &str) -> String {
    format!("local target\nfor b in save.bases() do target = b end\n{body}")
}

/// The reads degrade rather than raise, because three rows on the same handle
/// already do and a plugin author cannot predict a split between them. `id`
/// still answers: it comes off the entry key, which is the one thing still
/// readable.
#[test]
fn a_base_whose_record_cannot_be_read_answers_nil_rather_than_raising() {
    let mut harness = support::harness(CAPS);
    let id = break_the_last_bases_record(&mut harness);

    let (status, summary) = harness.run(&last_base(
        "return tostring(target.id) .. '|' .. tostring(target.name) .. '|' .. \
         tostring(target.area_range) .. '|' .. tostring(target.x) .. '|' .. \
         tostring(target.guild_id)",
    ));
    assert_eq!(status, RunStatus::Ok, "reading it must not raise: {summary:?}");
    assert_eq!(summary.as_deref(), Some(format!("{id}|nil|nil|nil|nil").as_str()));
}

/// The write side does not degrade, and must not: `apply_base_dto` writes both
/// writable rows into the base-camp record and silently does nothing when
/// there is none, so an accepted assignment would read back from the cache
/// inside the run and read nil again after the flush.
#[test]
fn assigning_to_a_base_whose_record_cannot_be_read_raises() {
    let mut harness = support::harness(CAPS);
    break_the_last_bases_record(&mut harness);

    for (field, value) in [("name", "'Nope'"), ("area_range", "10.0")] {
        let message = error_message(
            harness.run(&last_base(&format!("target.{field} = {value}\nreturn 'unreachable'"))).0,
        );
        assert!(message.contains(field), "must name the field, got {message:?}");
        assert!(
            message.contains("no base camp record"),
            "must say why nothing could be written, got {message:?}"
        );
    }
}

/// The other side of both writable rows admitting nil: the nil is a read-side
/// answer about the save's record, not a value an assignment can write.
#[test]
fn assigning_nil_raises() {
    let mut harness = support::harness(CAPS);
    for field in ["name", "area_range"] {
        let message = error_message(
            harness.run(&first_base(&format!("target.{field} = nil\nreturn 'unreachable'"))).0,
        );
        assert!(message.contains(field), "must name the field, got {message:?}");
        assert!(message.contains("nil"), "must name what it was given, got {message:?}");
    }
}

/// Zero and negative radii are written as given. Nothing in the game's data or
/// in this app establishes a legal range, so refusing them would be inventing
/// a rule; this pins that the code and the row's doc agree about that.
#[test]
fn a_zero_or_negative_area_range_is_accepted_and_written_as_given() {
    let mut harness = support::harness(CAPS);
    let id = a_base_id(&harness);
    let (status, summary) =
        harness.run(&first_base("target.area_range = -1.0\nreturn tostring(target.area_range)"));
    assert_eq!(status, RunStatus::Ok, "a negative radius must not be refused: {summary:?}");
    assert_eq!(summary.as_deref(), Some("-1.0"));
    assert_eq!(base_in_the_save(&mut harness, id).area_range, Some(-1.0));

    let mut zero = support::harness(CAPS);
    let (status, summary) =
        zero.run(&first_base("target.area_range = 0.0\nreturn tostring(target.area_range)"));
    assert_eq!(status, RunStatus::Ok, "a zero radius must not be refused: {summary:?}");
    assert_eq!(summary.as_deref(), Some("0.0"));
}

/// The third of the three dry-run combinations: a dry run that crosses a
/// mid-run flush. A real run's flush drains the cache, so the read afterwards
/// comes back out of the save; a dry run's must not, or the write it is
/// previewing would vanish from the rest of its own preview. Reading a pal
/// field is what reaches `flush`, and `ctx.dry_run` is what makes it return
/// before draining anything.
#[test]
fn a_dry_run_still_reads_back_its_own_write_across_a_mid_run_flush() {
    let mut harness = support::harness_dry(CAPS);
    let id = a_base_id(&harness);
    let (status, summary) = harness.run(&format!(
        "local target\n\
         for b in save.bases() do target = b break end\n\
         target.name = 'Preview Only'\n\
         local before = tostring(target.name)\n\
         {FORCE_FLUSH}\
         local after = tostring(target.name)\n\
         return before .. '|' .. after"
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("Preview Only|Preview Only"),
        "a dry run must keep reading its own pending write after a flush it never performed"
    );
    assert_ne!(
        base_in_the_save(&mut harness, id).name.as_deref(),
        Some("Preview Only"),
        "and nothing may have reached the save on the way"
    );
}

#[test]
fn a_string_field_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) =
        harness.run(&first_base("target.name = 'Renamed Base'\nreturn tostring(target.name)"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("Renamed Base"));
}

#[test]
fn a_number_field_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) =
        harness.run(&first_base("target.area_range = 1250.0\nreturn tostring(target.area_range)"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("1250.0"));
}

/// The save holds the radius as a 32-bit float. A wider value would arrive as
/// an infinity rather than as the number that was assigned.
#[test]
fn an_out_of_range_value_raises_and_names_the_field() {
    let mut harness = support::harness(CAPS);
    let message =
        error_message(harness.run(&first_base("target.area_range = 1e40\nreturn 'unreachable'")).0);
    assert!(message.contains("area_range"), "must name the field, got {message:?}");
    assert!(message.contains("1000000000000"), "must name the value, got {message:?}");
}

/// Same shape as a guild's: an empty name reaches the save's base writer as
/// "leave it alone", so the assignment would report success and change
/// nothing.
#[test]
fn assigning_an_empty_name_raises() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_base("target.name = ''\nreturn 'unreachable'")).0);
    assert!(message.contains("name"), "must name the field, got {message:?}");
    assert!(message.contains("empty"), "must say what it refused, got {message:?}");
}

/// Nothing in this app writes a base's position -- `BaseDto::location` is
/// output-only and the save's base writer never looks at it -- so the three
/// coordinates are honest read-only rows rather than setters that could not
/// have worked.
#[test]
fn assigning_a_coordinate_raises() {
    let mut harness = support::harness(CAPS);
    for axis in ["x", "y", "z"] {
        let message = error_message(
            harness.run(&first_base(&format!("target.{axis} = 1.0\nreturn 'unreachable'"))).0,
        );
        assert!(message.contains(axis), "must name the field, got {message:?}");
        assert!(message.contains("read-only"), "must say it is read-only, got {message:?}");
    }
}

#[test]
fn assigning_an_identity_field_raises() {
    let mut harness = support::harness(CAPS);
    for field in ["id", "guild_id"] {
        let message = error_message(
            harness.run(&first_base(&format!("target.{field} = 'x'\nreturn 'unreachable'"))).0,
        );
        assert!(message.contains(field), "must name the field, got {message:?}");
        assert!(message.contains("read-only"), "must say it is read-only, got {message:?}");
    }
}

#[test]
fn an_unknown_field_raises_rather_than_silently_succeeding() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_base("target.nmae = 'x'\nreturn 'unreachable'")).0);
    assert!(message.contains("nmae"), "must name the field, got {message:?}");
}

#[test]
fn a_wrong_typed_value_raises() {
    let mut harness = support::harness(CAPS);
    let message =
        error_message(harness.run(&first_base("target.area_range = 'wide'\nreturn 'unreachable'")).0);
    assert!(message.contains("area_range"), "must name the field, got {message:?}");
    assert!(message.contains("string"), "must name the type it got, got {message:?}");
}

/// `name` is read-write and the string is a legal one, so the only thing left
/// that can refuse this assignment is the capability gate. The negatives are
/// what stop the test passing for a different reason if that ever stops being
/// true.
#[test]
fn assignment_without_save_write_raises() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let message = error_message(harness.run(&first_base("target.name = 'Nope'\nreturn 'unreachable'")).0);
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must say which capability is missing, got {message:?}"
    );
    for wrong in ["unknown base field", "is read-only", "attempt to index", "cannot be empty"] {
        assert!(
            !message.contains(wrong),
            "the refusal must be the capability one, not {wrong:?}, got {message:?}"
        );
    }
}

/// The gate runs before the field name is resolved, so an ungranted plugin is
/// never told whether a field exists -- assigning a name that is not a base
/// field at all still reports the missing capability and nothing else. Order
/// the two the other way round and this is where it shows; with a real field
/// name the gate fires either way and the message is identical, so only an
/// unknown name can detect the reordering.
#[test]
fn an_ungranted_assignment_is_refused_before_the_field_name_is_resolved() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let message =
        error_message(harness.run(&first_base("target.no_such_field = 4\nreturn 'unreachable'")).0);
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must be refused for the capability, got {message:?}"
    );
    assert!(
        !message.contains("unknown base field"),
        "refusing an ungranted plugin by field name tells it which fields exist, got {message:?}"
    );
}

/// A dry run never flushes, so the value read back here cannot have come from
/// the save: it can only be the dirty DTO the write left in the cache.
#[test]
fn a_dry_run_reads_back_what_it_just_set() {
    let mut harness = support::harness_dry(CAPS);
    let (status, summary) = harness.run(&first_base(
        "target.name = 'Dry'\ntarget.area_range = 900.0\n\
         return tostring(target.name) .. ',' .. tostring(target.area_range)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("Dry,900.0"));
}

#[test]
fn a_dry_run_counts_each_accepted_assignment_and_a_real_run_counts_none() {
    let mut dry = support::harness_dry(CAPS);
    let (status, _) = dry.run(&first_base("target.name = 'A'\ntarget.name = 'B'\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(dry.counts().get("base.name").copied(), Some(2));

    let mut real = support::harness(CAPS);
    let (status, _) = real.run(&first_base("target.name = 'A'\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(real.counts().get("base.name"), None);
}

/// A refused assignment must contribute nothing to the preview.
#[test]
fn a_dry_run_does_not_count_a_refused_assignment() {
    let mut harness = support::harness_dry(CAPS);
    let (status, _) = harness.run(&first_base("pcall(function() target.name = '' end)\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(harness.counts().get("base.name"), None);
}

/// A dry run must reach the save with nothing at all, however convincingly it
/// reads back inside the run.
#[test]
fn a_dry_run_leaves_the_base_in_the_save_untouched() {
    let mut harness = support::harness_dry(CAPS);
    let id = a_base_id(&harness);
    let before = base_in_the_save(&mut harness, id).name;
    let (status, _) = harness.run(&first_base("target.name = 'Dry Run Only'\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(base_in_the_save(&mut harness, id).name, before);
}

/// The one thing a green field-assignment suite can still fail to say: that
/// the write reached the save at all. Read back through `get_guild_details`,
/// which knows nothing about this handle's cache.
#[test]
fn a_written_name_and_area_range_reach_the_save() {
    let mut harness = support::harness(CAPS);
    let id = a_base_id(&harness);
    let (status, _) =
        harness.run(&first_base("target.name = 'Saved Base'\ntarget.area_range = 2100.0\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);

    let dto = base_in_the_save(&mut harness, id);
    assert_eq!(dto.name.as_deref(), Some("Saved Base"), "the name must reach the base camp record");
    assert_eq!(dto.area_range, Some(2100.0), "the radius must reach the base camp record");
}

/// The cached value and the saved value have to be the same value, and the
/// save narrows the radius to a 32-bit float on the way in. A cache holding
/// the wider number the script assigned would answer one thing before a flush
/// and another after it.
#[test]
fn a_written_area_range_reads_the_same_on_both_sides_of_a_flush() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&format!(
        "local target\n\
         for b in save.bases() do target = b break end\n\
         local original = tostring(target.area_range)\n\
         target.area_range = 0.1\n\
         local cached = tostring(target.area_range)\n\
         {FORCE_FLUSH}\
         local saved = tostring(target.area_range)\n\
         return original .. '|' .. cached .. '|' .. saved"
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let mut parts = summary.split('|');
    let original = parts.next().expect("the original radius");
    let cached = parts.next().expect("the cached radius");
    let saved = parts.next().expect("the flushed radius");
    assert_ne!(original, cached, "the fixture base must not already carry this radius");
    assert_eq!(
        saved, cached,
        "the radius must read the same before and after the write reaches the save"
    );
}

#[test]
fn a_written_name_reads_the_same_on_both_sides_of_a_flush() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&format!(
        "local target\n\
         for b in save.bases() do target = b break end\n\
         local original = tostring(target.name)\n\
         target.name = 'Across The Flush'\n\
         local cached = tostring(target.name)\n\
         {FORCE_FLUSH}\
         local saved = tostring(target.name)\n\
         return original .. '|' .. cached .. '|' .. saved"
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let mut parts = summary.split('|');
    let original = parts.next().expect("the original name");
    let cached = parts.next().expect("the cached name");
    let saved = parts.next().expect("the flushed name");
    assert_ne!(original, "Across The Flush", "the fixture base must not already carry this name");
    assert_eq!(cached, "Across The Flush");
    assert_eq!(saved, cached, "the name must read the same before and after the write reaches the save");
}

/// Assigning a base field is non-structural, and everything obtained before
/// the write has to stay usable. A handle whose epoch no longer matches the
/// context's is refused outright, so a read through one of these succeeding is
/// exactly the statement that the mutation epoch did not move.
#[test]
fn a_base_write_leaves_every_live_handle_and_iterator_valid() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local base, guild, pal, container\n\
         for b in save.bases() do base = b break end\n\
         for g in save.guilds() do guild = g break end\n\
         for p in save.pals() do pal = p break end\n\
         for c in save.containers() do container = c break end\n\
         local bases = save.bases()\n\
         local first = bases()\n\
         base.name = 'Still Valid'\n\
         local out = {}\n\
         out[#out+1] = tostring(base.name)\n\
         out[#out+1] = tostring(guild.id ~= nil)\n\
         out[#out+1] = tostring(pal.instance_id ~= nil)\n\
         out[#out+1] = tostring(container.slot_count ~= nil)\n\
         out[#out+1] = tostring(first.id ~= nil)\n\
         out[#out+1] = tostring(bases() ~= nil)\n\
         return table.concat(out, ',')",
    );
    assert_eq!(status, RunStatus::Ok, "every handle and the part-consumed iterator must survive the write");
    assert_eq!(summary.as_deref(), Some("Still Valid,true,true,true,true,true"));
}

/// The census every container in the save reports, as one string.
const CONTAINER_CENSUS: &str = "local out = {}\n\
     for c in save.containers() do\n\
       local slots = {}\n\
       for s in c.slots() do\n\
         slots[#slots+1] = tostring(s.index) .. '=' .. tostring(s.item_id) .. 'x' .. tostring(s.count)\n\
       end\n\
       out[#out+1] = tostring(c.id) .. '|' .. tostring(c.slot_count) .. '|' .. table.concat(slots, ',')\n\
     end\n\
     return table.concat(out, ';')\n";

/// The hazard a base write carries: the save's own base writer walks
/// `BaseDto::storage_containers` into a write of every one of them, which
/// removes raw slot entries and drops dynamic items. None of that moves the
/// mutation epoch, so every handle and iterator would keep reporting itself
/// valid over containers that had been rewritten underneath them.
///
/// Deliberately blind to where the damage would be: it compares the whole
/// census -- every container in the save, every slot in each -- rather than
/// the storage containers of the one base being written.
#[test]
fn a_base_write_leaves_every_container_in_the_save_untouched() {
    let mut untouched = support::harness(CAPS);
    let (status, before) = untouched.run(&format!("{FORCE_FLUSH}{CONTAINER_CENSUS}"));
    assert_eq!(status, RunStatus::Ok);
    let before = before.expect("the census must produce a string");
    assert!(!before.is_empty(), "the fixture must hold containers for this to measure anything");

    let mut written = support::harness(CAPS);
    let (status, after) = written.run(&format!(
        "local target\n\
         for b in save.bases() do target = b break end\n\
         target.name = 'Cascade Check'\n\
         {FORCE_FLUSH}{CONTAINER_CENSUS}"
    ));
    assert_eq!(status, RunStatus::Ok);
    let after = after.expect("the census must produce a string");
    if after != before {
        let difference = before
            .split(';')
            .zip(after.split(';'))
            .find(|(untouched, written)| untouched != written)
            .map(|(untouched, written)| format!("before: {untouched}\nafter:  {written}"))
            .unwrap_or_else(|| {
                format!("the census gained or lost containers: {} -> {}", before.len(), after.len())
            });
        panic!("a base write must not touch any container\n{difference}");
    }
}

/// The base's own coordinates are not part of what a base write sends, and
/// nothing recomputes them from a changed radius either. A base handle read
/// after the write must report the position the save still holds.
#[test]
fn a_base_write_leaves_the_bases_position_untouched() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&format!(
        "local target\n\
         for b in save.bases() do target = b break end\n\
         local before = tostring(target.x) .. ',' .. tostring(target.y) .. ',' .. tostring(target.z)\n\
         target.area_range = 4000.0\n\
         {FORCE_FLUSH}\
         local after = tostring(target.x) .. ',' .. tostring(target.y) .. ',' .. tostring(target.z)\n\
         return before .. '|' .. after"
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let (before, after) = summary.split_once('|').expect("before|after");
    assert!(!before.contains("nil"), "the fixture base must have a resolvable position, got {before:?}");
    assert_eq!(before, after, "a base write must not move the base");
}
