mod support;

use psp_plugin::host::MAX_TABLE_NODES;
use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

#[test]
fn catalogs_lists_the_shipped_catalogs_sorted() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run(
        "local names = gamedata.catalogs()
         local sorted = true
         for i = 2, #names do if names[i] < names[i - 1] then sorted = false end end
         return string.format('%d,%s,%s', #names, tostring(sorted), tostring(names[1] ~= nil))",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<&str> = value.split(',').collect();
    assert_eq!(
        parts[0].parse::<usize>().expect("a count"),
        33,
        "the shipped game data has 33 top-level catalogs, the number docs/plugins.md quotes: {value}"
    );
    assert_eq!(parts[1], "true", "catalogs must come back sorted: {value}");
}

/// `GameData::load` recurses into `l10n/` and `ui/` alike, so the exclusion has
/// to be of nested names as such rather than of one prefix.
#[test]
fn catalogs_omits_every_nested_subtree() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run(
        "for _, name in ipairs(gamedata.catalogs()) do
           if name:find('/', 1, true) then return 'leaked: ' .. name end
         end
         return 'clean'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("clean"));
}

#[test]
fn keys_lists_a_catalogs_entries() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run(
        "local keys = gamedata.keys('pals')
         return tostring(#keys > 100)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true"));
}

#[test]
fn keys_answers_nil_for_an_unknown_catalog() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run("return tostring(gamedata.keys('no_such_catalog'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil"));
}

#[test]
fn the_gamedata_table_is_absent_without_its_capability() {
    let mut h = support::harness(&[Capability::SaveRead]);
    let (status, value) = h.run("return type(gamedata)");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil"));
}

/// A catalog that exists but is not an object has no keys -- which must read as
/// an empty table, not as the nil that means "no such catalog".
#[test]
fn keys_answers_an_empty_table_for_a_catalog_that_is_not_an_object() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run(
        "local keys = gamedata.keys('camps')
         return type(keys) .. ',' .. tostring(#keys)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("table,0"));
}

/// Reading fields off the entry, not just its type: a table that arrived
/// empty, or with its leaves dropped somewhere down the tree, would satisfy
/// `type(entry) == 'table'` and satisfy nothing an author wants.
#[test]
fn get_returns_one_entry_of_a_catalog() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run(
        "local keys = gamedata.keys('pals')
         local entry = gamedata.get('pals', keys[1])
         return type(entry) .. ',' .. type(entry.tribe) .. ',' ..
                tostring(#entry.tribe > 0) .. ',' .. type(entry.scaling.hp)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("table,string,true,number"));
}

#[test]
fn get_answers_nil_for_an_unknown_catalog_or_key() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run(
        "return tostring(gamedata.get('no_such_catalog')) .. ',' ..
                tostring(gamedata.get('pals', 'no_such_pal'))",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil,nil"));
}

/// The read-only guarantee, stated as the only thing that can actually be
/// observed: a mutation of what you were handed is invisible to the next read.
#[test]
fn mutating_a_returned_table_does_not_change_the_next_read() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run(
        "local keys = gamedata.keys('pals')
         local first = gamedata.get('pals', keys[1])
         first.__injected = 'mutated'
         local second = gamedata.get('pals', keys[1])
         return tostring(second.__injected)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil"));
}

/// Two fetches of the same entry must be independent tables, not one shared one.
#[test]
fn two_reads_of_the_same_entry_are_separate_tables() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run(
        "local keys = gamedata.keys('pals')
         return tostring(gamedata.get('pals', keys[1]) == gamedata.get('pals', keys[1]))",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn a_whole_catalog_fetch_returns_a_table() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run("return type(gamedata.get('elements'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("table"));
}

/// A synthetic catalog one entry past the node cap, not a real one, so this
/// stays true regardless of how the shipped data grows or shrinks.
#[test]
fn get_refuses_a_whole_catalog_fetch_that_is_too_large() {
    let nodes = MAX_TABLE_NODES + 1;
    let entries: Vec<String> = (0..nodes).map(|i| format!("\"k{i}\":{i}")).collect();
    let json = format!("{{{}}}", entries.join(","));
    let mut h = support::harness(&[Capability::GameData]).with_game_data_entries(&[("huge", &json)]);
    let (status, _) = h.run("return gamedata.get('huge')");
    match status {
        RunStatus::Error(message) => {
            assert!(message.contains("huge"), "names the catalog: {message}");
            assert!(message.contains("keys"), "points at gamedata.keys(): {message}");
            assert!(
                message.contains(&format!("{nodes} values (the limit is {MAX_TABLE_NODES})")),
                "reports the count against the limit, in the unit it counts: {message}"
            );
        }
        other => panic!("expected an error naming the oversized catalog, got {other:?}"),
    }
}

/// The cap is one rule for the whole of `gamedata.get`, not a rule for the
/// whole-catalog branch alone. The entry here is deliberately nested, so its
/// node count and the catalog's one top-level key cannot be confused for each
/// other -- the figure the message reports is the former.
#[test]
fn get_refuses_a_keyed_fetch_of_an_oversized_entry() {
    let keys = MAX_TABLE_NODES / 2 + 1;
    let nodes = keys * 2;
    let entries: Vec<String> = (0..keys).map(|i| format!("\"k{i}\":[{i}]")).collect();
    let json = format!("{{\"big\":{{{}}}}}", entries.join(","));
    let mut h =
        support::harness(&[Capability::GameData]).with_game_data_entries(&[("nested", &json)]);
    let (status, value) = h.run("return tostring(#gamedata.keys('nested'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("1"), "the catalog has exactly one top-level key");

    let (status, _) = h.run("return gamedata.get('nested', 'big')");
    match status {
        RunStatus::Error(message) => {
            assert!(message.contains("nested"), "names the catalog: {message}");
            assert!(message.contains("big"), "names the key: {message}");
            assert!(
                message.contains(&format!("{nodes} values (the limit is {MAX_TABLE_NODES})")),
                "reports the entry's node count, not its key count: {message}"
            );
        }
        other => panic!("expected an error naming the oversized entry, got {other:?}"),
    }
}

/// The cap exists to stop a fetch nobody could use, not to put the shipped
/// game data out of reach. Derived rather than named, so it keeps meaning the
/// same thing when a content patch changes which catalog is biggest.
#[test]
fn every_shipped_catalog_fetches_whole() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run(
        "local refused = {}
         for _, name in ipairs(gamedata.catalogs()) do
           local ok = pcall(gamedata.get, name)
           if not ok then refused[#refused + 1] = name end
         end
         return table.concat(refused, ',')",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        value.as_deref(),
        Some(""),
        "no shipped catalog may be too large for gamedata.get to return whole"
    );
}
