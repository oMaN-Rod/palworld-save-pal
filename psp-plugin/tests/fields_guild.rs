mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

const CAPS: &[Capability] = &[Capability::SaveRead, Capability::SaveWrite];

fn first_guild(body: &str) -> String {
    format!("local target\nfor g in save.guilds() do target = g break end\n{body}")
}

fn error_message(status: RunStatus) -> String {
    match status {
        RunStatus::Error(message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

/// Reading a pal field rebuilds the pal snapshot, and that is what flushes the
/// DTO cache out to the save -- which also drains the guild entry, so the next
/// read of a guild field comes back out of the guild tail rather than the
/// cache.
const FORCE_FLUSH: &str = "for p in save.pals() do local _ = p.level break end\n";

/// The guild as `psp-core` reads it back off the save, with no part of the
/// plugin host between. Everything about how this task caches, counts or
/// summarises a guild write is invisible here: if `update_guilds` did not
/// actually happen, this reports the old value.
fn guild_in_the_save(
    harness: &mut support::Harness,
    id: uuid::Uuid,
) -> psp_core::dto::guild::GuildDto {
    let data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("psp-plugin has a parent directory")
        .join("data/json");
    let game_data = psp_core::gamedata::GameData::load(&data_dir).expect("game data is checked in");
    psp_core::domain::guild::get_guild_details(harness.session_mut(), &game_data, id)
        .expect("the fixture guild must load")
        .expect("the fixture guild must exist")
}

fn a_guild_id(harness: &support::Harness) -> uuid::Uuid {
    *harness.session().guild_summary_order.first().expect("the corpus fixture has guilds")
}

#[test]
fn a_scalar_field_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) =
        harness.run(&first_guild("target.level = 7\nreturn tostring(target.level)"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("7"));
}

#[test]
fn a_string_field_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) =
        harness.run(&first_guild("target.name = 'Renamed'\nreturn tostring(target.name)"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("Renamed"));
}

#[test]
fn an_out_of_range_value_raises_and_names_the_field() {
    let mut harness = support::harness(CAPS);
    let message =
        error_message(harness.run(&first_guild("target.level = 1000000000000\nreturn 'unreachable'")).0);
    assert!(message.contains("level"), "must name the field, got {message:?}");
    assert!(message.contains("1000000000000"), "must name the value, got {message:?}");
}

/// `apply_guild_dto` reads a zero level as "leave it alone" and skips the
/// write, so the one number that looks like it sets a level is the one that
/// does nothing at all.
#[test]
fn assigning_a_zero_level_raises_rather_than_silently_doing_nothing() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_guild("target.level = 0\nreturn 'unreachable'")).0);
    assert!(message.contains("level"), "must name the field, got {message:?}");
    assert!(
        message.contains("leave the level alone"),
        "must say why this one number is refused, got {message:?}"
    );
}

/// Same shape as a zero level: an empty name reaches the save's guild writer
/// as "leave it alone", so the assignment would report success and change
/// nothing.
#[test]
fn assigning_an_empty_name_raises() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_guild("target.name = ''\nreturn 'unreachable'")).0);
    assert!(message.contains("name"), "must name the field, got {message:?}");
    assert!(message.contains("empty"), "must say what it refused, got {message:?}");
}

/// `player_count` is counted, not stored: there is nothing on the guild for a
/// write to land on, and a write that silently did nothing would be worse than
/// one that raises.
#[test]
fn assigning_a_derived_field_raises() {
    let mut harness = support::harness(CAPS);
    let message =
        error_message(harness.run(&first_guild("target.player_count = 5\nreturn 'unreachable'")).0);
    assert!(message.contains("player_count"), "must name the field, got {message:?}");
    assert!(message.contains("read-only"), "must say it is read-only, got {message:?}");
}

#[test]
fn assigning_an_identity_field_raises() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_guild("target.id = 'x'\nreturn 'unreachable'")).0);
    assert!(message.contains("id"), "must name the field, got {message:?}");
    assert!(message.contains("read-only"), "must say it is read-only, got {message:?}");
}

#[test]
fn an_unknown_field_raises_rather_than_silently_succeeding() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_guild("target.levle = 5\nreturn 'unreachable'")).0);
    assert!(message.contains("levle"), "must name the field, got {message:?}");
}

#[test]
fn a_wrong_typed_value_raises() {
    let mut harness = support::harness(CAPS);
    let message =
        error_message(harness.run(&first_guild("target.level = 'five'\nreturn 'unreachable'")).0);
    assert!(message.contains("level"), "must name the field, got {message:?}");
    assert!(message.contains("string"), "must name the type it got, got {message:?}");
}

/// `level` is read-write and `7` is in range, so the only thing left that can
/// refuse this assignment is the capability gate. The negatives are what stop
/// the test passing for a different reason if that ever stops being true.
#[test]
fn assignment_without_save_write_raises() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let message = error_message(harness.run(&first_guild("target.level = 7\nreturn 'unreachable'")).0);
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must say which capability is missing, got {message:?}"
    );
    for wrong in ["unknown guild field", "is read-only", "attempt to index", "must be between"] {
        assert!(
            !message.contains(wrong),
            "the refusal must be the capability one, not {wrong:?}, got {message:?}"
        );
    }
}

/// The gate runs before the field name is resolved, so an ungranted plugin is
/// never told whether a field exists -- assigning a name that is not a guild
/// field at all still reports the missing capability and nothing else. Order
/// the two the other way round and this is where it shows; with a real field
/// name the gate fires either way and the message is identical, so only an
/// unknown name can detect the reordering.
#[test]
fn an_ungranted_assignment_is_refused_before_the_field_name_is_resolved() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let message =
        error_message(harness.run(&first_guild("target.no_such_field = 4\nreturn 'unreachable'")).0);
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must be refused for the capability, got {message:?}"
    );
    assert!(
        !message.contains("unknown guild field"),
        "refusing an ungranted plugin by field name tells it which fields exist, got {message:?}"
    );
}

/// A dry run never flushes, so the value read back here cannot have come from
/// the save: it can only be the dirty DTO the write left in the cache.
#[test]
fn a_dry_run_reads_back_what_it_just_set() {
    let mut harness = support::harness_dry(CAPS);
    let (status, summary) = harness.run(&first_guild(
        "target.level = 9\ntarget.name = 'Dry'\n\
         return tostring(target.level) .. ',' .. tostring(target.name)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("9,Dry"));
}

#[test]
fn a_dry_run_counts_each_accepted_assignment_and_a_real_run_counts_none() {
    let mut dry = support::harness_dry(CAPS);
    let (status, _) = dry.run(&first_guild("target.level = 9\ntarget.level = 10\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(dry.counts().get("guild.level").copied(), Some(2));

    let mut real = support::harness(CAPS);
    let (status, _) = real.run(&first_guild("target.level = 9\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(real.counts().get("guild.level"), None);
}

/// `level` is declared a plain integer, not `integer|nil`: the guild tail
/// stores it with no way to record its absence. The read-back agreement test
/// enforces the type; this enforces the other half, that the nil an author
/// might expect from a union is not assignable either.
#[test]
fn assigning_nil_to_level_raises() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_guild("target.level = nil\nreturn 'unreachable'")).0);
    assert!(message.contains("level"), "must name the field, got {message:?}");
    assert!(message.contains("nil"), "must name what it was given, got {message:?}");
}

/// The third of the three dry-run combinations: a dry run that crosses a
/// mid-run flush. A real run's flush drains the cache, so the read afterwards
/// comes back out of the guild tail; a dry run's must not, or the write it is
/// previewing would vanish from the rest of its own preview. Reading a pal
/// field is what reaches `flush`, and `ctx.dry_run` is what makes it return
/// before draining anything.
#[test]
fn a_dry_run_still_reads_back_its_own_write_across_a_mid_run_flush() {
    let mut harness = support::harness_dry(CAPS);
    let id = a_guild_id(&harness);
    let (status, summary) = harness.run(&format!(
        "local target\n\
         for g in save.guilds() do target = g break end\n\
         target.name = 'Preview Only'\n\
         target.level = 23\n\
         local before = tostring(target.name) .. ',' .. tostring(target.level)\n\
         {FORCE_FLUSH}\
         local after = tostring(target.name) .. ',' .. tostring(target.level)\n\
         return before .. '|' .. after"
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("Preview Only,23|Preview Only,23"),
        "a dry run must keep reading its own pending write after a flush it never performed"
    );
    let dto = guild_in_the_save(&mut harness, id);
    assert_ne!(dto.name.as_deref(), Some("Preview Only"), "and nothing may have reached the save");
    assert_ne!(dto.base_camp_level, Some(23), "and nothing may have reached the save");
}

/// A refused assignment must contribute nothing to the preview.
#[test]
fn a_dry_run_does_not_count_a_refused_assignment() {
    let mut harness = support::harness_dry(CAPS);
    let (status, _) =
        harness.run(&first_guild("pcall(function() target.level = 0 end)\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(harness.counts().get("guild.level"), None);
}

/// A dry run must reach the save with nothing at all, however convincingly it
/// reads back inside the run.
#[test]
fn a_dry_run_leaves_the_guild_in_the_save_untouched() {
    let mut harness = support::harness_dry(CAPS);
    let id = a_guild_id(&harness);
    let before = guild_in_the_save(&mut harness, id).name;
    let (status, _) = harness.run(&first_guild("target.name = 'Dry Run Only'\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(guild_in_the_save(&mut harness, id).name, before);
}

/// The one thing a green field-assignment suite can still fail to say: that
/// the write reached the save at all. Read back through `get_guild_details`,
/// which knows nothing about this handle's cache or the session summary the
/// run refreshes on its way out.
#[test]
fn a_written_name_and_level_reach_the_save() {
    let mut harness = support::harness(CAPS);
    let id = a_guild_id(&harness);
    let (status, _) =
        harness.run(&first_guild("target.name = 'Saved Guild'\ntarget.level = 17\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);

    let dto = guild_in_the_save(&mut harness, id);
    assert_eq!(dto.name.as_deref(), Some("Saved Guild"), "the name must reach the guild tail");
    assert_eq!(dto.base_camp_level, Some(17), "the level must reach the guild tail");
}

/// The write only reaches the save at a flush, and the guild summary the app
/// keeps reading afterwards is session state that nothing recomputes -- so a
/// name that round-trips inside the run can still be lost to everything
/// outside it.
#[test]
fn a_written_name_and_level_survive_the_flush_into_the_guild_summary() {
    let mut harness = support::harness(CAPS);
    let id = a_guild_id(&harness);
    let (status, _) =
        harness.run(&first_guild("target.name = 'Summarised'\ntarget.level = 18\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    let summary = harness.session().guild_summaries.get(&id).expect("the fixture guild must survive");
    assert_eq!(summary.name, "Summarised");
    assert_eq!(summary.level, Some(18));
}

/// The cached value and the saved value have to be the same value. Reading
/// either side of a mid-run flush is the only way to see them disagree, and a
/// cache that answered with what the run believed it wrote rather than what
/// the save now holds is a defect this plan has shipped twice.
#[test]
fn a_written_name_reads_the_same_on_both_sides_of_a_flush() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&format!(
        "local target\n\
         for g in save.guilds() do target = g break end\n\
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
    assert_ne!(original, "Across The Flush", "the fixture guild must not already carry this name");
    assert_eq!(cached, "Across The Flush");
    assert_eq!(saved, cached, "the name must read the same before and after the write reaches the save");
}

/// Assigning a guild field is non-structural, and everything obtained before
/// the write has to stay usable. A handle whose epoch no longer matches the
/// context's is refused outright, so a read through one of these succeeding is
/// exactly the statement that the mutation epoch did not move.
#[test]
fn a_guild_write_leaves_every_live_handle_and_iterator_valid() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local guild, base, pal, container\n\
         for g in save.guilds() do guild = g break end\n\
         for b in save.bases() do base = b break end\n\
         for p in save.pals() do pal = p break end\n\
         for c in save.containers() do container = c break end\n\
         local bases = save.bases()\n\
         local first = bases()\n\
         guild.name = 'Still Valid'\n\
         local out = {}\n\
         out[#out+1] = tostring(guild.name)\n\
         out[#out+1] = tostring(base.id ~= nil)\n\
         out[#out+1] = tostring(pal.instance_id ~= nil)\n\
         out[#out+1] = tostring(container.slot_count ~= nil)\n\
         out[#out+1] = tostring(first.id ~= nil)\n\
         out[#out+1] = tostring(bases() ~= nil)\n\
         return table.concat(out, ',')",
    );
    assert_eq!(status, RunStatus::Ok, "every handle and the part-consumed iterator must survive the write");
    assert_eq!(summary.as_deref(), Some("Still Valid,true,true,true,true,true"));
}

/// The only way to obtain a `GuildDto` that carries this guild's bases and
/// chest is `get_guild_details`, and calling it puts the guild into
/// `loaded_guilds` as a side effect. So an empty `loaded_guilds` after a guild
/// write is the statement that the write was not built from a whole-guild
/// load -- the statement the two census tests below cannot make on a corpus
/// where re-applying a container happens to change nothing.
///
/// It pins a second thing worth pinning in its own right: `guild.delete()`
/// refuses a guild that is not in `loaded_guilds`, so a write that loaded the
/// guild whole would quietly turn a refused delete into an accepted one.
#[test]
fn a_guild_write_does_not_load_the_guild_whole() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_guild(
        "target.name = 'Not A Whole Load'\ntarget.level = 5\nlocal _ = target.chest_container_id\nreturn 'ok'",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert!(
        harness.session().loaded_guilds.is_empty(),
        "a guild field write must not load the guild whole: that is what carries its bases and \
         chest into the write, and what makes a previously refused guild.delete() succeed"
    );
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

/// The hazard a guild write carries that no other handle's does. The save's
/// own guild writer walks `GuildDto::bases` into a per-base write that rewrites
/// every storage container the base owns, and `GuildDto::guild_chest` into a
/// write of the guild's shared chest. Neither moves the mutation epoch, so
/// every handle and iterator would keep reporting itself valid over containers
/// that had been rewritten underneath them -- which is why the handle test
/// above is not enough on its own and this one compares the containers
/// themselves.
///
/// Deliberately blind to where the damage would be: it compares the whole
/// census -- every container in the save, every slot in each -- rather than
/// the base storage containers or the chest the cascade is known to reach.
#[test]
fn a_guild_write_leaves_every_container_in_the_save_untouched() {
    // Reading a pal field rebuilds the pal snapshot, and that is what flushes
    // the cache out to the save -- so the census below sees an applied write
    // rather than a pending one. In the untouched run it does nothing, and is
    // there only to keep the two scripts identical apart from the assignment.
    let flush = FORCE_FLUSH;

    let mut untouched = support::harness(CAPS);
    let (status, before) = untouched.run(&format!("{flush}{CONTAINER_CENSUS}"));
    assert_eq!(status, RunStatus::Ok);
    let before = before.expect("the census must produce a string");
    assert!(!before.is_empty(), "the fixture must hold containers for this to measure anything");

    let mut written = support::harness(CAPS);
    let (status, after) = written.run(&format!(
        "local target\n\
         for g in save.guilds() do target = g break end\n\
         target.name = 'Cascade Check'\n\
         {flush}{CONTAINER_CENSUS}"
    ));
    assert_eq!(status, RunStatus::Ok);
    let after = after.expect("the census must produce a string");
    if after != before {
        // The whole census is one string per container; printing all of it
        // would bury the one line that moved.
        let difference = before
            .split(';')
            .zip(after.split(';'))
            .find(|(untouched, written)| untouched != written)
            .map(|(untouched, written)| format!("before: {untouched}\nafter:  {written}"))
            .unwrap_or_else(|| {
                format!("the census gained or lost containers: {} -> {}", before.len(), after.len())
            });
        panic!("a guild write must not touch any container\n{difference}");
    }
}

/// The other half of the same cascade: a guild write must not reach the bases
/// either. `apply_base_dto` writes a base's name and working radius from the
/// DTO it is handed, so a base row moving under a guild write is exactly what
/// a populated `bases` looks like from the outside.
#[test]
fn a_guild_write_leaves_every_base_in_the_save_untouched() {
    const BASE_CENSUS: &str = "local out = {}\n\
         for b in save.bases() do\n\
           out[#out+1] = tostring(b.id) .. '|' .. tostring(b.name) .. '|' .. tostring(b.area_range)\n\
         end\n\
         return table.concat(out, ';')\n";

    let mut untouched = support::harness(CAPS);
    let (status, before) = untouched.run(&format!("{FORCE_FLUSH}{BASE_CENSUS}"));
    assert_eq!(status, RunStatus::Ok);
    let before = before.expect("the census must produce a string");
    assert!(!before.is_empty(), "the fixture must hold bases for this to measure anything");

    let mut written = support::harness(CAPS);
    let (status, after) = written.run(&format!(
        "local target\n\
         for g in save.guilds() do target = g break end\n\
         target.name = 'Cascade Check'\n\
         {FORCE_FLUSH}{BASE_CENSUS}"
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(after.as_deref(), Some(before.as_str()), "a guild write must not touch any base");
}
