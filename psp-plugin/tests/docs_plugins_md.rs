//! Pins `docs/plugins.md` against the host it describes: the Lua samples are
//! extracted and run, and the hand-written list of `player` rows that need the
//! `players` capability is compared against the field table that decides it.

mod support;

use std::collections::BTreeSet;
use std::path::PathBuf;

use psp_plugin::host::fields::player::read_requires_players;
use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;
use psp_plugin::PLAYER_FIELDS;
use uuid::Uuid;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("psp-plugin has a parent directory")
        .to_path_buf()
}

/// Line endings normalized: the checked-out files are CRLF on Windows and LF
/// elsewhere, and one test compares a doc sample byte for byte against the
/// plugin source it is quoted from.
fn read_normalized(relative: &str) -> String {
    let path = repo_root().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is checked in: {error}", path.display()));
    text.lines().collect::<Vec<_>>().join("\n")
}

fn docs() -> String {
    read_normalized("docs/plugins.md")
}

struct LuaBlock {
    /// 1-based line of the opening fence, for a failure that has to be found.
    line: usize,
    source: String,
}

fn lua_blocks(text: &str) -> Vec<LuaBlock> {
    let mut blocks = Vec::new();
    let mut open: Option<(usize, Vec<&str>)> = None;
    for (index, line) in text.lines().enumerate() {
        match &mut open {
            None => {
                if line == "```lua" {
                    open = Some((index + 1, Vec::new()));
                }
            }
            Some((start, body)) => {
                if line == "```" {
                    blocks.push(LuaBlock { line: *start, source: body.join("\n") });
                    open = None;
                } else {
                    body.push(line);
                }
            }
        }
    }
    assert!(open.is_none(), "docs/plugins.md has an unterminated ```lua fence");
    blocks
}

fn first_line(block: &LuaBlock) -> &str {
    block.source.lines().next().unwrap_or_default()
}

/// The blocks this file executes, each keyed by the start of its first line.
const EXECUTED: &[&str] =
    &["pal.level = 60", "local uids = {}", "function delete_empty_guilds()"];

/// The blocks this file does not execute, each with the reason. Running a
/// sample only proves something if the sample can fail; three of them cannot,
/// and forcing them to run would buy coverage that asserts nothing.
const SKIPPED: &[(&str, &str)] = &[
    (
        "-- mutates a copy; does nothing",
        "its first statement is wrong on purpose and its point is the difference between its \
         two halves, which executing it cannot assert; the behaviour itself is pinned by \
         fields_pal.rs::a_read_returns_a_snapshot_not_a_live_view",
    ),
    (
        "raw.get(target, path)",
        "a signature list rather than a program: the `-> bool` annotations are not Lua, and \
         `target` and `path` name nothing",
    ),
    (
        "for player in save.players() do",
        "an anti-pattern shown in order to be wrong, and it only raises once the raw.delete \
         has removed something; `SaveData.SomeArray` is absent from the corpus, so the run \
         would end without reaching the failure the prose describes",
    ),
];

/// The one block whose first line starts with `key`, panicking if the docs no
/// longer carry it -- a sample that has been renamed out from under this file
/// must be re-read, not silently skipped.
fn block(key: &str) -> String {
    let text = docs();
    let mut matching = lua_blocks(&text).into_iter().filter(|block| first_line(block).starts_with(key));
    let found = matching
        .next()
        .unwrap_or_else(|| panic!("docs/plugins.md has no lua block whose first line starts with {key:?}"));
    assert!(matching.next().is_none(), "more than one lua block starts with {key:?}");
    found.source
}

#[test]
fn every_lua_block_in_the_docs_is_executed_or_named_as_skipped() {
    let text = docs();
    let blocks = lua_blocks(&text);
    for block in &blocks {
        let first = first_line(block);
        let accounted = EXECUTED.iter().chain(SKIPPED.iter().map(|(key, _)| key)).any(|key| first.starts_with(key));
        assert!(
            accounted,
            "the lua block at docs/plugins.md:{} is neither executed by this file nor listed \
             in SKIPPED with a reason; its first line is {first:?}",
            block.line
        );
    }
    for key in EXECUTED.iter().chain(SKIPPED.iter().map(|(key, _)| key)) {
        assert!(
            blocks.iter().any(|block| first_line(block).starts_with(key)),
            "no lua block in docs/plugins.md starts with {key:?} any more"
        );
    }
    assert_eq!(
        blocks.len(),
        EXECUTED.len() + SKIPPED.len(),
        "every block matched a key and every key matched a block, so two blocks are sharing \
         one key -- either two of them start with the same text, or one key is a prefix of \
         another block's first line. One block is therefore unaccounted for."
    );
}

fn game_data() -> &'static psp_core::gamedata::GameData {
    static GAME_DATA: std::sync::OnceLock<psp_core::gamedata::GameData> = std::sync::OnceLock::new();
    GAME_DATA.get_or_init(|| {
        psp_core::gamedata::GameData::load(&repo_root().join("data/json")).expect("game data is checked in")
    })
}

/// A slot carrying no per-item record, which is the only kind `slot.item_id`
/// can be assigned on at all. Nothing visible from Lua distinguishes one, so
/// the sample's `slot` has to be picked out here.
fn a_plain_slot(harness: &mut support::Harness) -> (Uuid, i32) {
    let container_ids: Vec<Uuid> = harness
        .session()
        .item_container_map()
        .expect("the corpus fixture has item containers")
        .iter()
        .filter_map(|entry| {
            psp_core::props::get(psp_core::props::struct_props(&entry.key)?, &["ID"])
                .and_then(psp_core::props::as_uuid)
        })
        .collect();

    for id in container_ids {
        let session = harness.session_mut();
        let level = &session.level;
        let caches = &mut session.caches;
        let Some(container) =
            psp_core::domain::containers::read_item_container(level, caches, game_data(), id, "", None)
        else {
            continue;
        };
        let plain = container.slots.iter().find(|slot| {
            slot.dynamic_item.is_none() && !matches!(slot.static_id.as_deref(), Some("") | Some("None") | None)
        });
        if let Some(slot) = plain {
            return (id, slot.slot_index);
        }
    }
    panic!("the corpus fixture must hold an occupied slot with no per-item record")
}

/// One `handle.field = literal` line of the assignment sample.
struct Assignment {
    handle: String,
    field: String,
    expected: Expected,
}

enum Expected {
    /// A quoted string, which reads back exactly as written.
    Text(String),
    /// A number, compared after parsing: an integer literal assigned to a float
    /// row reads back as `3500.0`, which is the same value and a different
    /// string.
    Number(f64),
}

/// The handles the assignment test's preamble binds. A sample that reached for
/// another one would run against a nil global, so it is named here instead.
const BOUND_HANDLES: &[&str] = &["pal", "player", "guild", "base", "slot"];

/// Every assignment the sample makes, read out of the sample itself so that the
/// read-back covers exactly what the document assigns and cannot fall behind a
/// line added to it later.
///
/// A line this cannot read is a hard failure naming the line. Skipping one would
/// quietly restore the gap this parsing exists to close -- an assignment that
/// executes, and that nothing afterwards checks.
fn parse_assignments(sample: &str) -> Vec<Assignment> {
    let mut assignments = Vec::new();
    for (offset, raw) in sample.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let where_ = format!("line {} of the assignment sample, {line:?}", offset + 1);
        let unreadable = |why: &str| -> ! {
            panic!(
                "{where_}: {why}. This test builds its read-back from the sample and must not \
                 skip a line it cannot read, so either the line is a mistake or this parser \
                 needs widening."
            )
        };
        let Some((target, rhs)) = line.split_once(" = ") else {
            unreadable("not a `handle.field = literal` assignment")
        };
        let Some((handle, field)) = target.split_once('.') else {
            unreadable("the left-hand side is not `handle.field`")
        };
        if !is_identifier(handle) || !is_identifier(field) {
            unreadable("the left-hand side is not a plain `handle.field` pair");
        }
        if !BOUND_HANDLES.contains(&handle) {
            unreadable("this test's preamble binds no handle of that name");
        }
        let expected = match rhs.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
            Some(text) if !text.contains('"') => Expected::Text(text.to_string()),
            _ => match rhs.parse::<f64>() {
                Ok(number) => Expected::Number(number),
                Err(_) => unreadable("the right-hand side is neither a quoted string nor a number"),
            },
        };
        assignments.push(Assignment { handle: handle.to_string(), field: field.to_string(), expected });
    }
    assignments
}

/// The five-handle assignment sample. It is written as fragments -- no handle
/// is in scope in the doc -- so the block runs verbatim between a preamble that
/// binds one of each and a postamble that reads every assigned field back.
///
/// The postamble and the expected values are both built from the sample's own
/// lines. A thirteenth assignment added to the document is therefore read back
/// and checked without anyone touching this file; a hand-written list would
/// have executed it and looked away.
#[test]
fn the_assignment_sample_runs_and_reads_back_every_value_it_assigns() {
    let caps = &[Capability::SaveRead, Capability::SaveWrite, Capability::Players];
    let mut harness = support::harness(caps);
    let (container_id, slot_index) = a_plain_slot(&mut harness);
    let sample = block("pal.level = 60");
    let assignments = parse_assignments(&sample);
    assert!(!assignments.is_empty(), "the assignment sample must still assign something");

    let read_backs: Vec<String> = assignments
        .iter()
        .map(|assignment| format!("tostring({}.{})", assignment.handle, assignment.field))
        .collect();

    let (status, summary) = harness.run(&format!(
        "local pal, player, guild, base, slot\n\
         for p in save.pals() do pal = p break end\n\
         for p in save.players() do player = p break end\n\
         for g in save.guilds() do guild = g break end\n\
         for b in save.bases() do if b.area_range ~= nil then base = b break end end\n\
         for c in save.containers() do\n\
         \x20 if tostring(c.id) == '{container_id}' then\n\
         \x20   for s in c.slots() do if s.index == {slot_index} then slot = s break end end\n\
         \x20   break\n\
         \x20 end\n\
         end\n\
         assert(pal and player and guild and base and slot, 'the corpus must supply one of each handle')\n\
         {sample}\n\
         return table.concat({{ {} }}, '|')",
        read_backs.join(", ")
    ));

    assert_eq!(status, RunStatus::Ok, "the documented assignments must all be accepted");
    let summary = summary.expect("a string");
    let read_back: Vec<&str> = summary.split('|').collect();
    assert_eq!(
        read_back.len(),
        assignments.len(),
        "one read-back per assignment, unless a value contained the separator; got {summary:?}"
    );
    for (value, assignment) in read_back.iter().zip(&assignments) {
        let field = format!("{}.{}", assignment.handle, assignment.field);
        match &assignment.expected {
            Expected::Text(want) => {
                assert_eq!(value, want, "{field} must read back what the sample assigns it")
            }
            Expected::Number(want) => {
                let got: f64 = value.parse().unwrap_or_else(|_| {
                    panic!("{field} was assigned a number and read back {value:?}")
                });
                assert_eq!(got, *want, "{field} must read back what the sample assigns it");
            }
        }
    }
}

/// The two-pass shape the docs prescribe for raw writes driven by a
/// `save.players()` walk. Nothing is deleted -- the path is absent from the
/// corpus, and `raw.delete` answers `false` for that rather than raising --
/// so what this pins is that the shape itself still runs to the end over every
/// player, which is the whole of the claim the prose makes for it.
#[test]
fn the_two_pass_raw_delete_sample_walks_every_player_without_raising() {
    let caps = &[Capability::SaveRead, Capability::SaveRaw, Capability::Players];
    let mut harness = support::harness(caps);
    let players = harness.session().player_summary_order.len();
    assert!(players > 1, "the corpus must hold enough players for the shape to matter");

    let sample = block("local uids = {}");
    let (status, summary) = harness.run(&format!("{sample}\nreturn tostring(#uids)"));
    assert_eq!(status, RunStatus::Ok, "the documented two-pass shape must run to the end");
    assert_eq!(
        summary.as_deref(),
        Some(players.to_string().as_str()),
        "the first pass must collect every player's uid"
    );
}

/// The worked example, which the docs say is lifted from a bundled plugin.
/// Both halves of that claim are checked: it is still that plugin's code, and
/// it still runs.
#[test]
fn the_worked_example_is_the_bundled_plugins_own_code_and_still_runs() {
    let sample = block("function delete_empty_guilds()");
    let bundled = read_normalized("psp-app/src/bundled/pst.cleanup/main.lua");
    assert!(
        bundled.contains(&sample),
        "the worked example no longer appears in psp-app/src/bundled/pst.cleanup/main.lua, \
         which is where the docs say it comes from"
    );

    let caps = &[Capability::SaveRead, Capability::SaveWrite];
    let mut harness = support::harness(caps);
    // No guild in the corpus is empty, and a sample that selects nothing cannot
    // tell a working predicate from one reading a field that does not exist.
    // `guild.player_count` is read straight off this summary, so one guild is
    // put into that state directly.
    let id = *harness.session().guild_summary_order.first().expect("the corpus fixture has guilds");
    harness
        .session_mut()
        .guild_summaries
        .get_mut(&id)
        .expect("the first guild must have a summary")
        .player_count = 0;

    let (status, summary) = harness.run(&format!(
        "local expected = 0\n\
         for g in save.guilds() do if g.player_count == 0 then expected = expected + 1 end end\n\
         {sample}\n\
         local result = delete_empty_guilds()\n\
         return tostring(expected) .. '|' .. tostring(result.counts.guilds) .. '|' ..\n\
         \x20 tostring(result.counts.unresolved) .. '|' .. result.summary"
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let parts: Vec<&str> = summary.splitn(4, '|').collect();
    assert_eq!(parts.len(), 4, "expected expected|removed|unresolved|summary, got {summary:?}");
    let expected: i64 = parts[0].parse().expect("a count");
    let removed: i64 = parts[1].parse().expect("a count");
    let unresolved: i64 = parts[2].parse().expect("a count");
    assert_eq!(expected, 1, "exactly the one guild set empty above may be selected");
    assert_eq!(
        removed + unresolved,
        expected,
        "the sample's own predicate must select the same guild this run's census did"
    );
    assert!(
        parts[3].starts_with(&format!("Deleted {removed} empty guild(s)")),
        "the summary must report the count it deleted, got {:?}",
        parts[3]
    );
}

/// The `player` rows the docs list under their two origins. Which of the two
/// files a row comes from is not recorded anywhere in the code and so is not
/// checkable here; the union of the two lists is, because it is exactly the
/// set of rows that need `players` to read.
fn documented_gated_player_fields(text: &str) -> BTreeSet<String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with("- **From the player's own"))
        .expect("docs/plugins.md must still list the player rows by origin");
    let end = lines
        .iter()
        .position(|line| line.contains("need the `players` capability"))
        .expect("docs/plugins.md must still state how many rows need the capability");
    assert!(end > start, "the origin lists must come before the count that totals them");
    backticked_identifiers(&lines[start..end])
}

/// Every `` `like_this` `` token that reads as a field name. The prose in these
/// lines also backticks `.sav`, which is not one.
fn backticked_identifiers(lines: &[&str]) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for line in lines {
        for (index, part) in line.split('`').enumerate() {
            if index % 2 == 1 && is_identifier(part) {
                found.insert(part.to_string());
            }
        }
    }
    found
}

fn is_identifier(text: &str) -> bool {
    !text.is_empty()
        && text.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
        && text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

const NUMBER_WORDS: &[(&str, usize)] = &[
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
    ("seventeen", 17),
    ("eighteen", 18),
    ("nineteen", 19),
    ("twenty", 20),
];

/// The count the docs spell out in "**All eighteen need the `players`
/// capability**".
fn documented_gated_count(text: &str) -> usize {
    let line = text
        .lines()
        .find(|line| line.contains("need the `players` capability"))
        .expect("docs/plugins.md must still state how many rows need the capability");
    let word = line
        .split_whitespace()
        .find_map(|word| {
            let word = word.trim_matches(|c: char| !c.is_ascii_alphabetic()).to_ascii_lowercase();
            NUMBER_WORDS.iter().find(|(name, _)| *name == word).map(|(_, value)| *value)
        });
    word.unwrap_or_else(|| panic!("no spelled-out count in {line:?}"))
}

/// The docs name these rows one by one, and a row added to `PLAYER_FIELDS`
/// later would be gated by `read_requires_players` without anyone touching the
/// prose that claims to enumerate them.
#[test]
fn the_documented_player_rows_needing_players_are_the_ones_the_field_table_gates() {
    let text = docs();
    let documented = documented_gated_player_fields(&text);
    let gated: BTreeSet<String> = PLAYER_FIELDS
        .iter()
        .filter(|spec| read_requires_players(spec.name))
        .map(|spec| spec.name.to_string())
        .collect();

    assert_eq!(
        documented, gated,
        "docs/plugins.md's two origin lists must together name exactly the player rows that \
         need the players capability to read"
    );
    assert_eq!(
        documented_gated_count(&text),
        gated.len(),
        "docs/plugins.md's spelled-out count must match how many rows are gated"
    );
}

/// The `player` row of the handle table is written out by hand as well, and is
/// the first thing an author reads. `pals` is on it without being a field row:
/// it is the iterator factory, which the same row says it is.
#[test]
fn the_player_row_of_the_handle_table_names_every_field_and_nothing_else() {
    let text = docs();
    let row = text
        .lines()
        .find(|line| line.starts_with("| `player` |"))
        .expect("docs/plugins.md must still carry a player row in the handle table");
    let fields_cell = row.split('|').nth(2).expect("the row must have a fields cell");
    let documented = backticked_identifiers(&[fields_cell]);
    let mut expected: BTreeSet<String> = PLAYER_FIELDS.iter().map(|spec| spec.name.to_string()).collect();
    expected.insert("pals".to_string());
    assert_eq!(
        documented, expected,
        "the player row of the handle table must name every row of PLAYER_FIELDS, plus `pals`"
    );
}
