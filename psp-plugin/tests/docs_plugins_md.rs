//! Pins `docs/plugins.md` against the host it describes: the Lua samples are
//! extracted and run, and the hand-written list of `player` rows that need the
//! `players` capability is compared against the field table that decides it.

mod support;

use std::collections::BTreeSet;
use std::path::PathBuf;

use psp_plugin::host::fields::player::read_requires_players;
use psp_plugin::host::MAX_TABLE_NODES;
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
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
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

/// The `gamedata.get` size figures. Every one of them is measured against the
/// shipped `data/json`, which is refreshed with each content patch, so all of
/// them go stale silently -- including the claim that the section names every
/// fetch too large to satisfy.
const CAP_SECTION_START: &str = "**A `gamedata.get` can refuse.**";
const CAP_SECTION_END: &str = "### `save`";
const CALL_PREFIX: &str = "`gamedata.get(";
const OVER_CAP: &str = "**over the cap**";

/// The document's own words with every run of whitespace collapsed, so a
/// sentence that wraps across lines reads as one.
fn collapsed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cap_section(text: &str) -> String {
    let start = text
        .find(CAP_SECTION_START)
        .expect("docs/plugins.md must still explain when gamedata.get refuses");
    let end = text[start..].find(CAP_SECTION_END).map(|at| start + at).unwrap_or(text.len());
    collapsed(&text[start..end])
}

/// One `` `gamedata.get(...)` (N nodes) `` claim, with whether the prose that
/// follows it calls that fetch over the cap.
struct Quoted {
    catalog: String,
    key: Option<String>,
    nodes: usize,
    called_over_cap: bool,
}

/// Every size the cap section quotes. A `gamedata.get(` call written there
/// without one is a hard failure: an unmeasured example is exactly how a stale
/// figure would hide from this test.
fn quoted_sizes(section: &str) -> Vec<Quoted> {
    let mut spans: Vec<(usize, Quoted, usize)> = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = section[cursor..].find(CALL_PREFIX) {
        let call_start = cursor + offset;
        let args_start = call_start + CALL_PREFIX.len();
        let unreadable = |why: &str| -> ! {
            panic!(
                "the gamedata.get call at byte {call_start} of the cap section {why}. Every call \
                 written there must read as `gamedata.get('catalog'[, 'key'])` (N nodes) so that \
                 this test can measure it."
            )
        };
        let Some(args_end) = section[args_start..].find(")`").map(|at| args_start + at) else {
            unreadable("is never closed")
        };
        let mut args = section[args_start..args_end].split(", ").map(|arg| arg.trim_matches('\''));
        let Some(catalog) = args.next() else { unreadable("names no catalog") };
        let key = args.next().map(str::to_string);
        if args.next().is_some() {
            unreadable("takes more arguments than gamedata.get has");
        }
        let after = args_end + ")`".len();
        let Some(figure) = section[after..].strip_prefix(" (") else {
            unreadable("is not followed by a measured size in parentheses")
        };
        let Some(figure_len) = figure.find(')') else { unreadable("has an unclosed size") };
        let Some(digits) = figure[..figure_len].strip_suffix(" nodes") else {
            unreadable("has a parenthesis after it that does not read as `N nodes`")
        };
        let Ok(nodes) = digits.replace(',', "").parse::<usize>() else {
            unreadable("has a size that is not a number")
        };
        let tail = after + " (".len() + figure_len + 1;
        spans.push((
            call_start,
            Quoted { catalog: catalog.to_string(), key, nodes, called_over_cap: false },
            tail,
        ));
        cursor = tail;
    }

    let starts: Vec<usize> = spans.iter().map(|(start, _, _)| *start).collect();
    spans
        .into_iter()
        .enumerate()
        .map(|(index, (_, mut quoted, tail))| {
            let end = starts.get(index + 1).copied().unwrap_or(section.len());
            quoted.called_over_cap = section[tail..end].contains(OVER_CAP);
            quoted
        })
        .collect()
}

/// The host's own rule, called rather than restated, so the two cannot drift.
fn node_count(value: &serde_json::Value) -> usize {
    psp_plugin::host::gamedata::count_nodes(value, 0)
        .expect("the shipped game data is not nested past the host's depth limit")
}

fn top_level_catalogs() -> Vec<&'static str> {
    let mut names: Vec<&str> =
        game_data().entry_names().filter(|name| !name.contains('/')).collect();
    names.sort_unstable();
    names
}

/// Every fetch `gamedata.get` would have to refuse, as `(catalog, key)` with
/// `None` for a whole-catalog fetch.
fn fetches_over_the_cap() -> BTreeSet<(String, Option<String>)> {
    let mut over = BTreeSet::new();
    for name in top_level_catalogs() {
        let Some(value) = game_data().get(name) else { continue };
        if node_count(value) > MAX_TABLE_NODES {
            over.insert((name.to_string(), None));
        }
        if let Some(entries) = value.as_object() {
            for (key, entry) in entries {
                if node_count(entry) > MAX_TABLE_NODES {
                    over.insert((name.to_string(), Some(key.clone())));
                }
            }
        }
    }
    over
}

#[test]
fn the_documented_gamedata_sizes_are_the_shipped_game_datas_own() {
    let section = cap_section(&docs());
    assert_eq!(
        MAX_TABLE_NODES, 150_000,
        "the cap moved, and docs/plugins.md spells its old value out in digits"
    );
    assert!(
        section.contains("150,000 JSON nodes"),
        "the cap section must still quote the cap itself"
    );

    let quoted = quoted_sizes(&section);
    assert!(quoted.len() > 2, "the cap section must still measure the largest catalogs");

    for claim in &quoted {
        let call = match &claim.key {
            Some(key) => format!("gamedata.get('{}', '{key}')", claim.catalog),
            None => format!("gamedata.get('{}')", claim.catalog),
        };
        let catalog = game_data().get(&claim.catalog).unwrap_or_else(|| {
            panic!("docs/plugins.md measures {call}, and no such catalog ships")
        });
        let value = match &claim.key {
            Some(key) => catalog
                .as_object()
                .and_then(|entries| entries.get(key))
                .unwrap_or_else(|| panic!("docs/plugins.md measures {call}, which holds no such key")),
            None => catalog,
        };
        assert_eq!(
            claim.nodes,
            node_count(value),
            "docs/plugins.md quotes {} nodes for {call}",
            claim.nodes
        );
    }

    let named: BTreeSet<(String, Option<String>)> = quoted
        .iter()
        .filter(|claim| claim.called_over_cap)
        .map(|claim| (claim.catalog.clone(), claim.key.clone()))
        .collect();
    assert_eq!(
        named,
        fetches_over_the_cap(),
        "docs/plugins.md must mark as over the cap exactly the fetches gamedata.get refuses"
    );
}

/// The census in the `gamedata.keys` bullet: how many catalogs there are, and
/// which of them are JSON arrays and so answer an empty table.
#[test]
fn the_documented_catalog_census_matches_the_shipped_game_data() {
    let text = collapsed(&docs());
    let marker = " of the loaded game data's ";
    let at = text.find(marker).expect("docs/plugins.md must still count the top-level catalogs");
    let spelled = text[..at]
        .split_whitespace()
        .last()
        .expect("a spelled-out count before the catalog census")
        .to_ascii_lowercase();
    let documented_arrays = NUMBER_WORDS
        .iter()
        .find(|(word, _)| *word == spelled)
        .map(|(_, value)| *value)
        .unwrap_or_else(|| panic!("no spelled-out count in {spelled:?}"));
    let rest = &text[at + marker.len()..];
    let documented_total: usize = rest
        .split_whitespace()
        .next()
        .and_then(|word| word.parse().ok())
        .expect("the census must give the catalog total in digits");
    let open = rest.find('(').expect("the census must list the array catalogs in parentheses");
    let close = open + rest[open..].find(')').expect("the census list must be closed");
    let named = backticked_identifiers(&[&rest[open..close]]);

    let catalogs = top_level_catalogs();
    let arrays: BTreeSet<String> = catalogs
        .iter()
        .filter(|name| !game_data().get(name).is_some_and(serde_json::Value::is_object))
        .map(|name| (*name).to_string())
        .collect();

    assert_eq!(documented_total, catalogs.len(), "docs/plugins.md's catalog total must match");
    assert_eq!(documented_arrays, arrays.len(), "docs/plugins.md's array-catalog count must match");
    assert_eq!(named, arrays, "docs/plugins.md must name exactly the catalogs that are JSON arrays");
}

/// The render caps live in the browser, so nothing else in this crate can
/// catch the docs drifting from them; pinning the figure here at least makes
/// changing one a deliberate act.
#[test]
fn the_documented_render_caps_are_the_ones_the_renderer_uses() {
    let docs = docs();
    assert!(docs.contains("500 rows"), "the table row cap must be documented");
    assert!(
        docs.contains("Showing 500 of"),
        "the docs must show the exact wording the widget uses"
    );
}

/// Every widget type the manifest validator knows must be documented, or an
/// author has no way to learn it exists.
#[test]
fn every_widget_kind_is_documented() {
    let docs = docs();
    for kind in psp_plugin::manifest::WIDGET_KINDS {
        assert!(docs.contains(&format!("`{kind}`")), "widget type {kind} is undocumented");
    }
}

#[test]
fn every_entity_kind_is_documented() {
    let docs = docs();
    for kind in psp_plugin::manifest::ENTITY_KINDS {
        assert!(docs.contains(&format!("`{kind}`")), "entity kind {kind} is undocumented");
    }
}
