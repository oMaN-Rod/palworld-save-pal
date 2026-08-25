mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

const CAPS: &[Capability] = &[Capability::SaveRead, Capability::SaveWrite];
const CAPS_RAW: &[Capability] = &[Capability::SaveRead, Capability::SaveWrite, Capability::SaveRaw];

fn first_pal(body: &str) -> String {
    format!("local target\nfor p in save.pals() do target = p break end\n{body}")
}

#[test]
fn a_scalar_field_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_pal(
        "target.rank = 4\nreturn tostring(target.rank)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("4"));
}

/// `is_lucky` has an explicit `pal_field` arm sourced from the cached
/// boss/lucky flags, so this read never reaches the DTO-only fallthrough --
/// what this proves is that the write reaches `IsRarePal` through
/// `dto_cache`'s flush, since the read afterward is served by the ordinary
/// summary/index fast path and only reflects the write because that path
/// rebuilds from the save after a flush.
#[test]
fn a_write_to_a_field_the_summary_also_answers_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_pal(
        "target.is_lucky = true\nreturn tostring(target.is_lucky)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("true"));
}

/// Unlike `is_lucky`, `friendship_point` has no `pal_field` arm at all: this
/// read can only be answered by the fallthrough to `fields::pal::pal_get`.
#[test]
fn a_dto_only_field_with_no_pal_field_arm_round_trips_through_the_fallthrough() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_pal(
        "target.friendship_point = 12345\nreturn tostring(target.friendship_point)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("12345"));
}

/// The promoting write releases `character_id`'s pending-write claim and must
/// leave `is_lucky`'s standing. Releasing both would send this read to the pal
/// summary, which only a real flush ever updates -- so a dry run would answer
/// with the pre-write `false`.
#[test]
fn a_dry_run_reads_back_the_is_lucky_it_just_set_on_a_plain_pal() {
    let mut harness = support::harness_dry(CAPS);
    let (status, summary) = harness.run(
        "local target
         for p in save.pals() do
           if not p.is_lucky and p.character_id:sub(1, 5) ~= 'BOSS_' then target = p break end
         end
         assert(target ~= nil, 'the fixture must hold a plain, unprefixed pal')
         target.is_lucky = true
         return tostring(target.is_lucky)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("true"), "a dry run must see its own is_lucky write");
}

/// `PAL_FIELDS` is the single source of truth a later task's agreement test
/// reads: `guild_id`/`base_id` must actually be rows on it, not just
/// reachable through some other path.
#[test]
fn pal_fields_declares_guild_id_and_base_id() {
    let names: Vec<&str> = psp_plugin::PAL_FIELDS.iter().map(|f| f.name).collect();
    assert!(names.contains(&"guild_id"), "{names:?}");
    assert!(names.contains(&"base_id"), "{names:?}");
    for name in ["guild_id", "base_id"] {
        let spec = psp_plugin::PAL_FIELDS.iter().find(|f| f.name == name).expect("present");
        assert_eq!(spec.access, psp_plugin::Access::ReadOnly, "{name} has no PalDto field to write into");
    }
}

#[test]
fn an_out_of_range_value_raises_and_names_the_field() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_pal("target.talent_hp = 500\nreturn 'unreachable'"));
    match status {
        RunStatus::Error(message) => {
            assert!(message.contains("talent_hp"), "must name the field, got {message:?}");
            assert!(message.contains("500"), "must name the value, got {message:?}");
        }
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn an_unknown_field_raises_rather_than_silently_succeeding() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_pal("target.rnak = 5\nreturn 'unreachable'"));
    match status {
        RunStatus::Error(message) => assert!(message.contains("rnak"), "got {message:?}"),
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn assigning_an_identity_field_raises() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_pal(
        "target.instance_id = 'x'\nreturn 'unreachable'",
    ));
    match status {
        RunStatus::Error(message) => {
            assert!(message.contains("instance_id"), "got {message:?}");
            assert!(message.contains("read"), "must say it is read-only, got {message:?}");
        }
        other => panic!("expected an error, got {other:?}"),
    }
}

/// `apply_pal_dto` writes `SlotIndex` with no occupancy or collision guard
/// (`psp-core/src/domain/pal.rs:614`), so a direct write here could silently
/// collide with another pal's slot; the field stays read-only rather than
/// exposing that hazard.
#[test]
fn assigning_storage_slot_raises() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_pal("target.storage_slot = 0\nreturn 'unreachable'"));
    match status {
        RunStatus::Error(message) => {
            assert!(message.contains("storage_slot"), "got {message:?}");
            assert!(message.contains("read"), "must say it is read-only, got {message:?}");
        }
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn a_wrong_typed_value_raises() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_pal("target.rank = 'five'\nreturn 'unreachable'"));
    assert!(matches!(status, RunStatus::Error(_)));
}

/// `apply_pal_dto` derives boss-ness from `character_id`'s `BOSS_` prefix once
/// `IsRarePal` is gone, and `character_id` itself is read-only through this
/// handle. Refusing the demoting write outright would leave `is_lucky`
/// advertised read-write while its `false` value stays unreachable for every
/// pal that is actually lucky -- so the write instead strips exactly the
/// prefix it put there, and only that prefix, leaving the pal plain rather
/// than silently boss-flagged.
#[test]
fn setting_is_lucky_false_strips_the_boss_prefix_so_the_pal_ends_up_plain() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_pal(
        "target.is_lucky = true\n\
         local _ = target.character_id\n\
         target.is_lucky = false\n\
         local stripped = target.character_id:sub(1, 5) ~= 'BOSS_'\n\
         return tostring(target.is_lucky) .. '|' .. tostring(target.is_boss) .. '|' .. tostring(stripped)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("false|false|true"),
        "must be neither lucky nor a boss, and character_id must no longer start with BOSS_"
    );
}

/// The promoting write is the other half of `apply_is_lucky`: it never
/// touches `character_id`, so the `BOSS_` prefix appears only when
/// `apply_pal_dto` derives it at a flush. Serving `character_id` from the
/// still-unflushed DTO here would report a bare id the save never holds, so
/// this pins the read against `raw.get` on the flushed `CharacterID`.
#[test]
fn setting_is_lucky_true_reads_the_character_id_that_reaches_the_save() {
    let mut harness = support::harness(CAPS_RAW);
    let (status, summary) = harness.run(
        "local id
         for p in save.pals() do
           if not p.is_lucky and p.character_id:sub(1, 5) ~= 'BOSS_' then id = p.instance_id break end
         end
         assert(id ~= nil, 'the fixture must hold a plain, unprefixed pal')

         local target
         for p in save.pals() do if p.instance_id == id then target = p break end end
         target.is_lucky = true
         local through_the_api = target.character_id

         local count = raw.len('level', 'worldSaveData.CharacterSaveParameterMap')
         local index
         for i = 0, count - 1 do
           local this_id = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. i .. '].key.InstanceId')
           if this_id == id then index = i break end
         end
         assert(index ~= nil, 'pal entry not found in CharacterSaveParameterMap')
         local saved = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. index ..
             '].value.RawData.SaveParameter.CharacterID')
         return through_the_api .. '|' .. saved",
    );
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.unwrap_or_default();
    let Some((through_the_api, saved)) = summary.split_once('|') else {
        panic!("expected 'api|saved', got {summary:?}")
    };
    assert!(saved.starts_with("BOSS_"), "the flush must have planted the prefix, got {saved:?}");
    assert_eq!(
        through_the_api, saved,
        "character_id read after is_lucky = true must be the id the save actually holds"
    );
}

/// A pal's `written` set is cumulative for the whole run, so the demoting
/// write's `character_id` claim outlives the write that earned it. A
/// re-promote in the same run un-strips nothing -- `apply_pal_dto` re-adds the
/// prefix at the flush -- so the claim has to be released, not merely left
/// unextended.
#[test]
fn re_promoting_after_a_demote_reads_the_character_id_that_reaches_the_save() {
    let mut harness = support::harness(CAPS_RAW);
    let (status, summary) = harness.run(
        "local id
         for p in save.pals() do
           if p.is_lucky and p.character_id:sub(1, 5) == 'BOSS_' then id = p.instance_id break end
         end
         assert(id ~= nil, 'the fixture must hold a lucky, BOSS_-prefixed pal')

         local target
         for p in save.pals() do if p.instance_id == id then target = p break end end
         target.is_lucky = false
         target.is_lucky = true
         local through_the_api = target.character_id

         local count = raw.len('level', 'worldSaveData.CharacterSaveParameterMap')
         local index
         for i = 0, count - 1 do
           local this_id = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. i .. '].key.InstanceId')
           if this_id == id then index = i break end
         end
         assert(index ~= nil, 'pal entry not found in CharacterSaveParameterMap')
         local saved = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. index ..
             '].value.RawData.SaveParameter.CharacterID')
         return through_the_api .. '|' .. saved",
    );
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.unwrap_or_default();
    let Some((through_the_api, saved)) = summary.split_once('|') else {
        panic!("expected 'api|saved', got {summary:?}")
    };
    assert!(saved.starts_with("BOSS_"), "the re-promoting flush must have restored the prefix, got {saved:?}");
    assert_eq!(
        through_the_api, saved,
        "character_id read after a demote then a re-promote must be the id the save actually holds"
    );
}

/// `pals.json` has 35 keys that literally begin with `BOSS_` -- human/NPC
/// pals like `BOSS_Male_People`, whose own key keeps the prefix
/// (`boss_male_people`, not `male_people`). Stripping it the way
/// `setting_is_lucky_false_strips_the_boss_prefix_so_the_pal_ends_up_plain`
/// does for an ordinary boosted pal would fabricate a species that may not
/// exist in `pals.json` at all -- so this must be refused, not silently
/// applied. `raw.set` plants the NPC id directly since no fixture pal is
/// naturally one of these 35.
#[test]
fn setting_is_lucky_false_is_refused_for_a_boss_prefixed_npc_species() {
    let mut harness = support::harness(CAPS_RAW);
    let (status, _) = harness.run(
        "local id
         for p in save.pals() do id = p.instance_id break end

         local count = raw.len('level', 'worldSaveData.CharacterSaveParameterMap')
         local index
         for i = 0, count - 1 do
           local this_id = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. i .. '].key.InstanceId')
           if this_id == id then index = i break end
         end
         assert(index ~= nil, 'pal entry not found in CharacterSaveParameterMap')
         local char_path = 'worldSaveData.CharacterSaveParameterMap[' .. index ..
             '].value.RawData.SaveParameter.CharacterID'
         raw.set('level', char_path, 'BOSS_Male_People')

         local target
         for p in save.pals() do if p.instance_id == id then target = p break end end
         target.is_lucky = true
         target.is_lucky = false
         return 'unreachable'",
    );
    match status {
        RunStatus::Error(message) => {
            assert!(message.contains("BOSS_Male_People"), "must name the species, got {message:?}");
            assert!(
                message.contains("species"),
                "must explain the prefix is part of the species name, got {message:?}"
            );
        }
        other => panic!("expected an error, got {other:?}"),
    }
}

/// `GameData::load` tolerates a missing or malformed `pals.json` by leaving
/// the catalog empty rather than erroring. Against an empty catalog,
/// `format_character_key` would strip `boss_` unconditionally, making
/// `BOSS_Male_People` look exactly like a safe strip too -- so this must be
/// refused on the catalog being unavailable, not silently allowed through.
#[test]
fn setting_is_lucky_false_is_refused_when_the_species_catalog_is_unavailable() {
    let mut harness = support::harness(CAPS).with_empty_game_data();
    let (status, _) = harness.run(&first_pal(
        "target.is_lucky = true\n\
         local _ = target.character_id\n\
         target.is_lucky = false\n\
         return 'unreachable'",
    ));
    match status {
        RunStatus::Error(message) => {
            assert!(message.contains("is_lucky"), "got {message:?}");
            assert!(
                message.contains("catalog"),
                "must explain the species catalog is unavailable, got {message:?}"
            );
        }
        other => panic!("expected an error, got {other:?}"),
    }
}

/// A `Boss_`-cased prefix fails an exact `starts_with("BOSS_")`, so without a
/// case-insensitive gate this write would neither refuse nor strip --
/// leaving `is_lucky = false` with the prefix untouched, which
/// `apply_pal_dto`'s own case-sensitive/insensitive asymmetry then turns
/// into a doubled `BOSS_Boss_Foxparks` id on flush. This must refuse
/// instead.
#[test]
fn setting_is_lucky_false_is_refused_for_a_mixed_case_boss_prefix() {
    let mut harness = support::harness(CAPS_RAW);
    let (status, _) = harness.run(
        "local id
         for p in save.pals() do id = p.instance_id break end

         local target
         for p in save.pals() do if p.instance_id == id then target = p break end end
         target.is_lucky = true

         local count = raw.len('level', 'worldSaveData.CharacterSaveParameterMap')
         local index
         for i = 0, count - 1 do
           local this_id = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. i .. '].key.InstanceId')
           if this_id == id then index = i break end
         end
         assert(index ~= nil, 'pal entry not found in CharacterSaveParameterMap')
         local char_path = 'worldSaveData.CharacterSaveParameterMap[' .. index ..
             '].value.RawData.SaveParameter.CharacterID'
         raw.set('level', char_path, 'Boss_Foxparks')

         target.is_lucky = false
         return 'unreachable'",
    );
    match status {
        RunStatus::Error(message) => {
            assert!(message.contains("is_lucky"), "got {message:?}");
            assert!(
                message.contains("species"),
                "must be the species-name refusal, not a type error or the catalog one, got {message:?}"
            );
        }
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn assignment_without_save_write_raises() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let (status, _) = harness.run(&first_pal("target.rank = 4\nreturn 'unreachable'"));
    match status {
        RunStatus::Error(message) => assert!(
            message.contains("save.write"),
            "an ungranted write must say which capability is missing, got {message:?}"
        ),
        other => panic!("expected an error, got {other:?}"),
    }
}
