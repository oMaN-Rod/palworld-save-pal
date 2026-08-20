mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

const CAPS: &[Capability] = &[Capability::SaveRead, Capability::SaveWrite, Capability::GameData];

const SCRIPTS: &[(&str, &str)] = &[
    (
        "empty guilds",
        "return tostring(save.guilds():delete_where(function(g) return g.player_count == 0 end))",
    ),
    (
        "every guild",
        "return tostring(save.guilds():delete_where(function() return true end))",
    ),
    (
        "invalid pals",
        "return tostring(save.pals():delete_where(function(p) \
           return not gamedata.is_valid_pal(p.character_id) end))",
    ),
    (
        "no matches",
        "return tostring(save.pals():delete_where(function() return false end))",
    ),
    (
        "every player",
        "return tostring(save.players():delete_where(function() return true end))",
    ),
    (
        "invalid items",
        "return tostring(save.clear_slots_where(function(s) \
           return s.item_id ~= nil and not gamedata.is_valid_item(s.item_id) end))",
    ),
    (
        "every occupied slot",
        "return tostring(save.clear_slots_where(function(s) return s.item_id ~= nil end))",
    ),
    (
        "grow an occupied container",
        "local target, capacity
         for c in save.containers() do
           for s in c.slots() do
             if s.item_id ~= nil then target = c.id capacity = c.slot_count break end
           end
           if target then break end
         end
         for c in save.containers() do
           if c.id == target then return tostring(c.set_slot_count(capacity + 5)) end
         end
         return 'no occupied container in fixture'",
    ),
];

#[test]
fn the_bulk_slot_clear_is_not_vacuous_and_its_dry_run_predicts_the_real_count() {
    let script = "return tostring(save.clear_slots_where(function(s) return s.item_id ~= nil end))";

    let mut wet = support::harness(CAPS);
    let (status, real) = wet.run(script);
    assert_eq!(status, RunStatus::Ok);
    let real: i64 = real.expect("a count").parse().expect("an integer");
    assert!(real > 0, "the fixture must have occupied slots for this test to mean anything");

    let mut dry = support::harness_dry(CAPS);
    let (status, predicted) = dry.run(script);
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(predicted.as_deref(), Some(real.to_string().as_str()));
}

#[test]
fn a_dry_run_reports_the_same_count_the_real_run_produces() {
    for (label, script) in SCRIPTS {
        let mut dry = support::harness_dry(CAPS);
        let (dry_status, dry_count) = dry.run(script);
        assert_eq!(dry_status, RunStatus::Ok, "{label} dry run");

        let mut wet = support::harness(CAPS);
        let (wet_status, wet_count) = wet.run(script);
        assert_eq!(wet_status, RunStatus::Ok, "{label} real run");

        assert_eq!(dry_count, wet_count, "{label}: dry-run count must match the real count");
    }
}

#[test]
fn a_dry_run_leaves_the_session_byte_identical() {
    for (label, script) in SCRIPTS {
        let mut h = support::harness_dry(CAPS);
        let before = h.session().level_gvas_bytes().expect("serializes");
        let (status, _) = h.run(script);
        assert_eq!(status, RunStatus::Ok, "{label}");
        let after = h.session().level_gvas_bytes().expect("serializes");
        assert_eq!(before, after, "{label}: a dry run must not change a single byte");
    }
}

#[test]
fn a_dry_run_records_what_it_would_have_done_in_the_counts() {
    let mut h = support::harness_dry(CAPS);
    let (status, _) = h.run(SCRIPTS[1].1);
    assert_eq!(status, RunStatus::Ok);
    let total: i64 = h.counts().values().sum();
    assert!(total > 0, "a dry run that would delete guilds must count them");
}

#[test]
fn a_dry_run_keeps_handles_valid_because_nothing_moved() {
    let mut h = support::harness_dry(CAPS);
    let (status, value) = h.run(
        "local first for g in save.guilds() do first = g break end
         save.guilds():delete_where(function() return true end)
         return tostring(first.name ~= nil)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true"));
}
