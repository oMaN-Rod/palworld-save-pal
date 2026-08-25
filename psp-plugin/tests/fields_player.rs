mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

/// Reading a player row the summary cannot answer needs `players` as well;
/// the two tests that probe the capability boundary itself grant less.
const CAPS: &[Capability] =
    &[Capability::SaveRead, Capability::SaveWrite, Capability::Players];

fn first_player(body: &str) -> String {
    format!("local target\nfor p in save.players() do target = p break end\n{body}")
}

fn error_message(status: RunStatus) -> String {
    match status {
        RunStatus::Error(message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn a_scalar_field_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_player("target.level = 42\nreturn tostring(target.level)"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("42"));
}

/// `level` is one of the seven fields the player summary answers directly, so
/// this read is served by that fast path -- and it only reflects the write
/// because the write recorded the field name, which routes the read to the
/// cached DTO instead. `technology_points` has no summary arm at all, so it can
/// only ever be answered by the field table.
#[test]
fn a_field_the_summary_does_not_carry_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) =
        harness.run(&first_player("target.technology_points = 77\nreturn tostring(target.technology_points)"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("77"));
}

#[test]
fn an_out_of_range_value_raises_and_names_the_field() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_player("target.level = 500\nreturn 'unreachable'")).0);
    assert!(message.contains("level"), "must name the field, got {message:?}");
    assert!(message.contains("500"), "must name the value, got {message:?}");
}

/// `pal_count` is counted, not stored: there is nothing on the player for a
/// write to land on, and a write that silently did nothing would be worse than
/// one that raises.
#[test]
fn assigning_a_derived_field_raises() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_player("target.pal_count = 5\nreturn 'unreachable'")).0);
    assert!(message.contains("pal_count"), "must name the field, got {message:?}");
    assert!(message.contains("read-only"), "must say it is read-only, got {message:?}");
}

#[test]
fn assigning_an_identity_field_raises() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_player("target.uid = 'x'\nreturn 'unreachable'")).0);
    assert!(message.contains("uid"), "must name the field, got {message:?}");
    assert!(message.contains("read-only"), "must say it is read-only, got {message:?}");
}

#[test]
fn an_unknown_field_raises_rather_than_silently_succeeding() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_player("target.levle = 5\nreturn 'unreachable'")).0);
    assert!(message.contains("levle"), "must name the field, got {message:?}");
}

#[test]
fn a_wrong_typed_value_raises() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_player("target.level = 'five'\nreturn 'unreachable'")).0);
    assert!(message.contains("level"), "must name the field, got {message:?}");
    assert!(message.contains("string"), "must name the type it got, got {message:?}");
}

/// `level` is read-write and `42` is in range, so the only thing left that can
/// refuse this assignment is the capability gate. The negatives are what stop
/// the test passing for a different reason if that ever stops being true.
#[test]
fn assignment_without_save_write_raises() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let message = error_message(harness.run(&first_player("target.level = 42\nreturn 'unreachable'")).0);
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must say which capability is missing, got {message:?}"
    );
    for wrong in ["unknown player field", "is read-only", "attempt to index", "must be between"] {
        assert!(
            !message.contains(wrong),
            "the refusal must be the capability one, not {wrong:?}, got {message:?}"
        );
    }
}

/// The gate runs before the field name is resolved, so an ungranted plugin is
/// never told whether a field exists -- assigning a name that is not a player
/// field at all still reports the missing capability and nothing else. Order
/// the two the other way round and this is where it shows; with a real field
/// name the gate fires either way and the message is identical, so only an
/// unknown name can detect the reordering.
#[test]
fn an_ungranted_assignment_is_refused_before_the_field_name_is_resolved() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let message = error_message(harness.run(&first_player("target.no_such_field = 4\nreturn 'unreachable'")).0);
    assert!(
        message.contains("requires the save.write capability"),
        "an ungranted write must be refused for the capability, got {message:?}"
    );
    assert!(
        !message.contains("unknown player field"),
        "refusing an ungranted plugin by field name tells it which fields exist, got {message:?}"
    );
}

#[test]
fn a_list_field_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_player(
        "target.technologies = { 'Foo', 'Bar' }\nreturn table.concat(target.technologies, ',')",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("Foo,Bar"));
}

#[test]
fn a_map_field_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_player(
        "target.status_point_list = { max_hp = 7, attack = 3 }\n\
         local points = target.status_point_list\n\
         return tostring(points.max_hp) .. ',' .. tostring(points.attack)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("7,3"));
}

/// `apply_status_points` looks each key up in its name map and simply skips one
/// it does not recognise, so an unrecognised key reaches the save as nothing at
/// all rather than as an error. Refusing it here is the only place that
/// difference can be reported.
#[test]
fn an_unknown_status_point_key_raises_and_names_the_key() {
    let mut harness = support::harness(CAPS);
    let message = error_message(
        harness.run(&first_player("target.status_point_list = { hp = 7 }\nreturn 'unreachable'")).0,
    );
    assert!(message.contains("\"hp\""), "must name the key it refused, got {message:?}");
    assert!(message.contains("max_hp"), "must list the keys it does know, got {message:?}");
}

/// `capture_rate` is a base stat with no extended-stat row, so the two maps
/// have genuinely different key sets and a single shared check would accept it
/// for both.
#[test]
fn each_status_point_map_validates_against_its_own_key_set() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_player(
        "local base = pcall(function() target.status_point_list = { capture_rate = 1 } end)\n\
         local ext = pcall(function() target.ext_status_point_list = { capture_rate = 1 } end)\n\
         return tostring(base) .. ',' .. tostring(ext)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("true,false"));
}

/// `apply_player_dto` compares the incoming nickname against this exact string
/// and *removes* the `NickName` property on a match rather than storing it, so
/// the one assignment that looks like it sets a name is the one that clears it.
#[test]
fn assigning_the_saves_nameless_placeholder_raises_rather_than_clearing_the_name() {
    let mut harness = support::harness(CAPS);
    let message = error_message(
        harness
            .run(&first_player(
                "local head = string.sub(target.uid, 1, 8)\n\
                 target.name = '\\u{1F977} (' .. head .. ')'\n\
                 return 'unreachable'",
            ))
            .0,
    );
    assert!(message.contains("name"), "must name the field, got {message:?}");
    assert!(
        message.contains("placeholder"),
        "must say why this one string is refused, got {message:?}"
    );
}

/// An empty name is stored literally, but the player summary substitutes its
/// own placeholder for an empty `NickName` -- so the same read would answer one
/// thing this run and another the next time the save is opened.
#[test]
fn assigning_an_empty_name_raises() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_player("target.name = ''\nreturn 'unreachable'")).0);
    assert!(message.contains("name"), "must name the field, got {message:?}");
    assert!(message.contains("empty"), "must say what it refused, got {message:?}");
}

/// The write only reaches the save at a flush, and the player summary the app
/// keeps reading afterwards is session state that nothing recomputes -- so a
/// name that round-trips inside the run can still be lost to everything outside
/// it. This reads the summary after the run has ended.
#[test]
fn a_written_name_survives_the_flush_into_the_player_summary() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_player("target.name = 'Renamed'\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    let uid = harness.a_player_uid();
    let summary = harness.session().player_summaries.get(&uid).expect("the fixture player must survive");
    assert_eq!(summary.nickname, "Renamed");
}

#[test]
fn a_written_level_survives_the_flush_into_the_player_summary() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_player("target.level = 33\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    let uid = harness.a_player_uid();
    let summary = harness.session().player_summaries.get(&uid).expect("the fixture player must survive");
    assert_eq!(summary.level, Some(33));
}

/// A dry run never flushes, so the value read back here cannot have come from
/// the save: it can only be the dirty DTO the write left in the cache.
#[test]
fn a_dry_run_reads_back_what_it_just_set() {
    let mut harness = support::harness_dry(CAPS);
    let (status, summary) = harness.run(&first_player(
        "target.level = 44\ntarget.technologies = { 'Foo' }\n\
         return tostring(target.level) .. ',' .. table.concat(target.technologies, ',')",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("44,Foo"));
}

#[test]
fn a_dry_run_counts_each_accepted_assignment_and_a_real_run_counts_none() {
    let mut dry = support::harness_dry(CAPS);
    let (status, _) = dry.run(&first_player("target.level = 44\ntarget.level = 45\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(dry.counts().get("player.level").copied(), Some(2));

    let mut real = support::harness(CAPS);
    let (status, _) = real.run(&first_player("target.level = 44\nreturn 'ok'"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(real.counts().get("player.level"), None);
}

/// A refused assignment must contribute nothing to the preview.
#[test]
fn a_dry_run_does_not_count_a_refused_assignment() {
    let mut harness = support::harness_dry(CAPS);
    let (status, _) = harness.run(&first_player(
        "pcall(function() target.level = 900 end)\nreturn 'ok'",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(harness.counts().get("player.level"), None);
}

/// Assigning a player field is non-structural, and everything obtained before
/// the write has to stay usable. A handle whose epoch no longer matches the
/// context's is refused outright, so a read through one of these succeeding is
/// exactly the statement that the mutation epoch did not move.
#[test]
fn a_player_write_leaves_every_live_handle_and_iterator_valid() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local player, pal, container\n\
         for p in save.players() do player = p break end\n\
         for p in save.pals() do pal = p break end\n\
         for c in save.containers() do container = c break end\n\
         local pals = save.pals()\n\
         local first = pals()\n\
         player.level = 21\n\
         local out = {}\n\
         out[#out+1] = tostring(player.level)\n\
         out[#out+1] = tostring(pal.instance_id ~= nil)\n\
         out[#out+1] = tostring(container.slot_count ~= nil)\n\
         out[#out+1] = tostring(first.instance_id ~= nil)\n\
         out[#out+1] = tostring(pals() ~= nil)\n\
         return table.concat(out, ',')",
    );
    assert_eq!(status, RunStatus::Ok, "every handle and the part-consumed iterator must survive the write");
    assert_eq!(summary.as_deref(), Some("21,true,true,true,true"));
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

/// The id of the fixture player's own common item container, and the size the
/// essential container's `AdditionalInventory_` slots would make
/// `apply_item_container_dto` recompute for it.
fn player_common_container(harness: &mut support::Harness) -> uuid::Uuid {
    let uid = harness.a_player_uid();
    let data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("psp-plugin has a parent directory")
        .join("data/json");
    let game_data = psp_core::gamedata::GameData::load(&data_dir).expect("game data is checked in");
    let dto = psp_core::domain::player::get_player_details(
        harness.session_mut(),
        &game_data,
        uid,
        &psp_core::progress::null_progress(),
    )
    .expect("the fixture player must load")
    .expect("the fixture player must exist");
    dto.common_container.expect("the fixture player must have a common container").id
}

/// The hazard a player write carries that no other handle's does: the DTO it
/// writes back carries the player's five item containers, and applying one of
/// those removes raw slot entries and resizes the paired common container to a
/// size recomputed from scratch. None of that moves the mutation epoch, so
/// every handle and iterator would keep reporting itself valid over containers
/// that had been rewritten underneath them -- which is why the handle test
/// above is not enough on its own and this one compares the containers
/// themselves.
///
/// Deliberately blind to where the damage would be: it compares the whole
/// census -- every container in the save, every slot in each -- rather than the
/// one container the resize is known to target.
///
/// Both runs grow that container first. Without that, the fixture's own common
/// container already holds exactly the size the recomputation would arrive at
/// (48, from the essential container's two `AdditionalInventory_` slots), so
/// the resize would fire and change nothing, and the test would pass whether or
/// not the containers were kept out of the write.
#[test]
fn a_player_write_leaves_every_container_in_the_save_untouched() {
    let grow = {
        let mut probe = support::harness(CAPS);
        let common = player_common_container(&mut probe);
        format!(
            "local common\n\
             for c in save.containers() do if tostring(c.id) == '{common}' then common = c break end end\n\
             assert(common ~= nil, 'the common container must be reachable')\n\
             assert(common.set_slot_count(60), 'the grow must succeed')\n"
        )
    };
    // Reading a pal field rebuilds the pal snapshot, and that is what flushes
    // the cache out to the save -- so the census below sees an applied write
    // rather than a pending one. In the untouched run it does nothing, and is
    // there only to keep the two scripts identical apart from the assignment.
    let flush = "for p in save.pals() do local _ = p.level break end\n";

    let mut untouched = support::harness(CAPS);
    let (status, before) = untouched.run(&format!("{grow}{flush}{CONTAINER_CENSUS}"));
    assert_eq!(status, RunStatus::Ok);
    let before = before.expect("the census must produce a string");
    assert!(!before.is_empty(), "the fixture must hold containers for this to measure anything");

    let mut written = support::harness(CAPS);
    let (status, after) = written.run(&format!(
        "{grow}\
         local target\n\
         for p in save.players() do target = p break end\n\
         target.level = 21\n\
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
        panic!("a player write must not touch any container\n{difference}");
    }
}

/// Renders a map field as a sorted `key=value` list, so two reads of the same
/// map can be compared as strings whatever order `pairs` hands them over in.
const DUMP_HELPER: &str = "local function dump(t)\n\
     local keys = {}\n\
     for k in pairs(t) do keys[#keys+1] = k end\n\
     table.sort(keys)\n\
     local out = {}\n\
     for _, k in ipairs(keys) do out[#out+1] = k .. '=' .. tostring(t[k]) end\n\
     return table.concat(out, ',')\n\
   end\n";

/// Reading a pal field rebuilds the pal snapshot, and that is what flushes the
/// DTO cache out to the save -- which also drains the player entry, so the next
/// read of a player field comes back out of the save rather than the cache.
const FORCE_FLUSH: &str = "for p in save.pals() do local _ = p.level break end\n";

fn parse_dump(dump: &str) -> Vec<(&str, &str)> {
    dump.split(',')
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.split_once('=').unwrap_or_else(|| panic!("expected key=value, got {entry}")))
        .collect()
}

/// The save's own writer *merges* the stat-point list: it visits only the keys
/// the map it is handed carries, so a key left out keeps whatever row it
/// already had. A row that simply replaced the cached map would therefore claim
/// a one-key map while the save still held all seventeen, and the same read
/// would answer differently either side of a flush -- which is exactly the
/// invariant `pal_release_field`'s doc states the cache depends on ("the cached
/// value already equals what a flush would write").
///
/// So this reads the map twice, once from the cache and once from the save, and
/// requires the two to agree; and separately requires the keys the assignment
/// left out to have gone to zero rather than kept their old values, since two
/// reads agreeing on a merge would satisfy the first check alone.
#[test]
fn a_partial_status_point_map_replaces_the_whole_map_on_both_sides_of_a_flush() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&format!(
        "{DUMP_HELPER}\
         local target\n\
         for p in save.players() do target = p break end\n\
         local original = dump(target.status_point_list)\n\
         target.status_point_list = {{ max_hp = 7 }}\n\
         local cached = dump(target.status_point_list)\n\
         {FORCE_FLUSH}\
         local saved = dump(target.status_point_list)\n\
         return original .. ' :: ' .. cached .. ' :: ' .. saved"
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let mut parts = summary.split(" :: ");
    let original = parts.next().expect("the original map");
    let cached = parts.next().expect("the cached map");
    let saved = parts.next().expect("the flushed map");

    assert!(
        original.contains("max_hp=") && !original.contains("max_hp=7"),
        "the fixture must carry stat rows this assignment actually changes, got {original:?}"
    );
    assert_eq!(
        cached, saved,
        "the map must read the same before and after the write reaches the save"
    );
    for (key, value) in parse_dump(cached) {
        let expected = if key == "max_hp" { "7" } else { "0" };
        assert_eq!(
            value, expected,
            "assigning {{ max_hp = 7 }} must leave {key} at {expected}, not merge into the \
             existing map; got {cached:?}"
        );
    }
}

/// The half of the replacement that cannot be decided from the cached map
/// alone. A stat the save carries no row for stays absent when it is left out,
/// because the save's writer will not append a row for a zero -- but once an
/// earlier assignment in the same run has put a positive value for that stat
/// into the cache, the cache can no longer tell "no row, assigned zero" from
/// "has a row, assigned zero", and would answer `0` before the flush and `nil`
/// after it. The load-time key set is what keeps the two the same.
#[test]
fn a_stat_the_save_has_no_row_for_stays_absent_after_being_set_and_unset_in_one_run() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&format!(
        "local target\n\
         for p in save.players() do target = p break end\n\
         assert(\n\
           target.status_point_list.stamina_reduction == nil,\n\
           'this test needs a stat the fixture player has no row for'\n\
         )\n\
         target.status_point_list = {{ stamina_reduction = 5 }}\n\
         local set = tostring(target.status_point_list.stamina_reduction)\n\
         target.status_point_list = {{ max_hp = 1 }}\n\
         local cached = tostring(target.status_point_list.stamina_reduction)\n\
         {FORCE_FLUSH}\
         local saved = tostring(target.status_point_list.stamina_reduction)\n\
         return set .. '|' .. cached .. '|' .. saved"
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("5|nil|nil"),
        "a rowless stat must read back what was assigned, then read nil on both sides of the \
         flush once the next assignment leaves it out"
    );
}

/// A player whose entry carries no `Level` byte read `nil` before this handle
/// gained writable fields, and must still. The failure mode this pins is a
/// fall-through: routing the summary's `nil` on to the field table sends it to
/// a `PlayerDto` row, which loads the player's `.sav` and answers with
/// `build_player_dto`'s own default instead -- turning the `nil` into a number
/// and paying a disk read to do it.
///
/// The corpus has no such player, so the summary is put into that state
/// directly. That is the whole of the condition: `player_field` reads
/// `summary.level`, and nothing else distinguishes the case.
#[test]
fn a_player_the_save_records_no_level_for_reads_nil() {
    let mut harness = support::harness(CAPS);
    let uid = harness.a_player_uid();
    harness
        .session_mut()
        .player_summaries
        .get_mut(&uid)
        .expect("the fixture player must have a summary")
        .level = None;

    let (status, summary) = harness.run(&first_player("return tostring(target.level)"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("nil"),
        "a level-less player must read nil, not a default loaded out of their own save file"
    );
}

/// The other half of `level` being declared `integer|nil`: the nil is reachable
/// on the read side only. There is no way to record a player with no level, so
/// the assignment raises rather than pretending to clear it.
#[test]
fn assigning_nil_to_level_raises() {
    let mut harness = support::harness(CAPS);
    let message = error_message(harness.run(&first_player("target.level = nil\nreturn 'unreachable'")).0);
    assert!(message.contains("level"), "must name the field, got {message:?}");
    assert!(message.contains("nil"), "must name what it was given, got {message:?}");
}

/// Which rows cost a load, measured rather than asserted in a doc comment. The
/// seven summary-backed rows must leave `loaded_players` untouched however many
/// players are walked -- that is what makes `save.players()` cheap -- and the
/// first read of any other row must put that one player in it.
///
/// Reading `p.uid` through the handle also goes through `player_get`'s summary
/// reader now, so this covers the whole no-load path and not just the two rows
/// `player_field` shortcuts.
#[test]
fn only_a_row_the_summary_cannot_answer_loads_the_players_own_save() {
    let mut harness = support::harness(&[Capability::SaveRead, Capability::Players]);
    let (status, seen) = harness.run(
        "local n = 0\n\
         for p in save.players() do\n\
           local _ = p.uid, p.name, p.level, p.guild_id, p.pal_count, p.last_online, p.last_online_ts\n\
           n = n + 1\n\
         end\n\
         return tostring(n)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_ne!(seen.as_deref(), Some("0"), "the fixture must hold players for this to measure anything");
    assert!(
        harness.session().loaded_players.is_empty(),
        "reading only summary-backed rows must not pull any player's own save off disk, however \
         many players are walked"
    );

    let (status, _) = harness.run(&first_player("return tostring(target.technology_points)"));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        harness.session().loaded_players.len(),
        1,
        "the first read of a row the summary cannot answer must load exactly that one player"
    );
}

/// Exactly the seven rows a `save.read` grant reached before this handle gained
/// writable fields, spelled out as the historical fact they are rather than
/// derived from anything this code still computes. Every one is answered from
/// the `PlayerSummary` the session already holds.
const READABLE_WITH_SAVE_READ_ALONE: &[&str] =
    &["uid", "name", "level", "guild_id", "pal_count", "last_online", "last_online_ts"];

/// The capability boundary, probed row by row across the whole table. A row
/// added later that answers from the player's own `PlayerDto` is gated by
/// default, and a row that somehow escaped the gate shows up here as an eighth
/// success -- which is why the expectation is the historical list and not a
/// list this test derives from `PLAYER_FIELDS` (that would compare the code to
/// itself).
#[test]
fn only_the_rows_that_were_always_save_read_are_readable_without_the_players_capability() {
    let mut script = String::from(
        "local target\n\
         for p in save.players() do target = p break end\n\
         assert(target ~= nil, 'the fixture must hold a player')\n\
         local out = {}\n",
    );
    for spec in psp_plugin::PLAYER_FIELDS {
        script.push_str(&format!(
            "do\n  local ok, err = pcall(function() return target['{n}'] end)\n             \x20 out[#out+1] = '{n}=' .. (ok and 'read' or tostring(err))\nend\n",
            n = spec.name
        ));
    }
    script.push_str("return table.concat(out, ';')");

    let mut harness = support::harness(&[Capability::SaveRead]);
    let (status, value) = harness.run(&script);
    assert_eq!(status, RunStatus::Ok, "the probe script itself must run cleanly: {value:?}");
    let value = value.expect("a string");

    let mut readable: Vec<&str> = Vec::new();
    let mut probed = 0usize;
    for entry in value.split(';') {
        let (name, outcome) = entry.split_once('=').unwrap_or_else(|| panic!("expected name=outcome, got {entry}"));
        probed += 1;
        if outcome == "read" {
            readable.push(name);
            continue;
        }
        assert!(
            outcome.contains("requires the players capability"),
            "a refused read must name the capability it needs, got {name}: {outcome}"
        );
        assert!(
            outcome.contains(name),
            "a refused read must name the field it refused, got {name}: {outcome}"
        );
    }

    assert_eq!(probed, psp_plugin::PLAYER_FIELDS.len(), "every row must be probed exactly once");
    assert_eq!(
        readable, READABLE_WITH_SAVE_READ_ALONE,
        "a save.read grant must reach the rows it always reached, and no others"
    );
    assert!(
        harness.session().loaded_players.is_empty(),
        "a refused read must not have loaded the player's own save on the way to refusing"
    );
}

/// The other side of the same boundary: granting `players` turns every one of
/// those refusals into a value. Without this, deleting the whole read path
/// would satisfy the test above just as well as gating it would.
#[test]
fn granting_players_makes_every_player_row_readable() {
    let mut script = String::from(
        "local target\n\
         for p in save.players() do target = p break end\n\
         local out = {}\n",
    );
    for spec in psp_plugin::PLAYER_FIELDS {
        script.push_str(&format!(
            "do\n  local ok = pcall(function() return target['{n}'] end)\n             \x20 out[#out+1] = '{n}=' .. tostring(ok)\nend\n",
            n = spec.name
        ));
    }
    script.push_str("return table.concat(out, ';')");

    let mut harness = support::harness(&[Capability::SaveRead, Capability::Players]);
    let (status, value) = harness.run(&script);
    assert_eq!(status, RunStatus::Ok, "the probe script itself must run cleanly: {value:?}");
    let refused: Vec<&str> = value
        .as_deref()
        .expect("a string")
        .split(';')
        .filter(|entry| entry.ends_with("=false"))
        .collect();
    assert!(refused.is_empty(), "with players granted no row may be refused, got {refused:?}");
}

/// Writing is deliberately not gated on `players`: rewriting a player's own
/// save under `save.write` alone is what the setter this handle replaced
/// already did, so gating the write would be a new restriction rather than a
/// restored one. Reading the value back is a different act and stays gated.
#[test]
fn writing_a_gated_row_needs_no_players_capability_but_reading_it_back_does() {
    let mut harness = support::harness(&[Capability::SaveRead, Capability::SaveWrite]);
    let (status, summary) = harness.run(&first_player(
        "target.technology_points = 77\n         local ok, err = pcall(function() return target.technology_points end)\n         return tostring(ok) .. '|' .. tostring(err)",
    ));
    assert_eq!(status, RunStatus::Ok, "the write itself must be allowed");
    let summary = summary.expect("a string");
    assert!(summary.starts_with("false|"), "reading it back must still be refused, got {summary:?}");
    assert!(
        summary.contains("requires the players capability"),
        "and the refusal must name the capability, got {summary:?}"
    );
}

/// `ext_status_point_list` shares `replacement_status_points` with
/// `status_point_list`, and a shared mechanism covered on one side only is how
/// the other side rots. Its key set is the smaller one, and the fixture happens
/// to carry a row already sitting at zero, so this also pins that a zero with a
/// row survives as a zero rather than being dropped.
#[test]
fn a_partial_ext_status_point_map_replaces_the_whole_map_on_both_sides_of_a_flush() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&format!(
        "{DUMP_HELPER}\
         local target\n\
         for p in save.players() do target = p break end\n\
         local original = dump(target.ext_status_point_list)\n\
         target.ext_status_point_list = {{ attack = 3 }}\n\
         local cached = dump(target.ext_status_point_list)\n\
         {FORCE_FLUSH}\
         local saved = dump(target.ext_status_point_list)\n\
         return original .. ' :: ' .. cached .. ' :: ' .. saved"
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let mut parts = summary.split(" :: ");
    let original = parts.next().expect("the original map");
    let cached = parts.next().expect("the cached map");
    let saved = parts.next().expect("the flushed map");

    assert!(
        original.contains("attack=") && !original.contains("attack=3"),
        "the fixture must carry extended-stat rows this assignment actually changes, got {original:?}"
    );
    assert_eq!(
        cached, saved,
        "the extended map must read the same before and after the write reaches the save"
    );
    for (key, value) in parse_dump(cached) {
        let expected = if key == "attack" { "3" } else { "0" };
        assert_eq!(
            value, expected,
            "assigning {{ attack = 3 }} must leave {key} at {expected}, not merge into the \
             existing map; got {cached:?}"
        );
    }
}
