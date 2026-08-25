mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;
use support::FORCE_FLUSH;

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

/// A real run's flush drains the pal cache, so the read afterwards is answered
/// from the rebuilt snapshot; a dry run's must not, or the write it is
/// previewing disappears from the rest of its own preview. The flush point is
/// reached the moment the run takes another `save.pals()` iterator, which the
/// assignment itself has made unavoidable by dropping the snapshot.
#[test]
fn a_dry_run_still_reads_back_its_own_write_across_a_mid_run_flush() {
    let mut harness = support::harness_dry(CAPS);
    let (status, summary) = harness.run(&format!(
        "local target\n\
         for p in save.pals() do target = p break end\n\
         local original = tostring(target.level) .. ',' .. tostring(target.talent_hp)\n\
         target.level = 41\n\
         target.talent_hp = 77\n\
         local before = tostring(target.level) .. ',' .. tostring(target.talent_hp)\n\
         {FORCE_FLUSH}\
         local after = tostring(target.level) .. ',' .. tostring(target.talent_hp)\n\
         return original .. '|' .. before .. '|' .. after"
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let parts: Vec<&str> = summary.split('|').collect();
    assert_eq!(parts.len(), 3, "expected original|before|after, got {summary:?}");
    assert_ne!(parts[0], "41,77", "the fixture pal must not already hold the values assigned");
    assert_eq!(parts[1], "41,77", "the write must read back before the flush");
    assert_eq!(
        parts[2], "41,77",
        "a dry run must keep reading its own pending write after a flush it never performed"
    );
    assert_eq!(harness.dto_flush_count(), 0, "and no dry run may write a pal back to the save");
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

/// `nickname` is declared `string|nil`, so an author reading `psp.lua` sees a
/// type that admits nil and gets no editor complaint for assigning one. The nil
/// is a read-side answer only, and the refusal is the only thing that says so.
#[test]
fn assigning_nil_to_nickname_raises() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_pal("target.nickname = nil\nreturn 'unreachable'"));
    match status {
        RunStatus::Error(message) => {
            assert!(message.contains("nickname"), "must name the field, got {message:?}");
            assert!(message.contains("nil"), "must name what it was given, got {message:?}");
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
/// case-insensitive gate this write would neither refuse nor strip, and would
/// report a demotion it did not perform -- `is_lucky = false` with the marker
/// prefix still on the id. What that casing means is a guess, so this must
/// refuse rather than pick a reading.
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

/// `rank` is read-write and `4` is in range, so the only thing left that can
/// refuse this assignment is the capability gate. The negatives are what stop
/// the test passing for a different reason if that ever stops being true.
#[test]
fn assignment_without_save_write_raises() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let (status, _) = harness.run(&first_pal("target.rank = 4\nreturn 'unreachable'"));
    match status {
        RunStatus::Error(message) => {
            assert!(
                message.contains("requires the save.write capability"),
                "an ungranted write must say which capability is missing, got {message:?}"
            );
            for wrong in ["unknown pal field", "is read-only", "attempt to index", "must be between"] {
                assert!(
                    !message.contains(wrong),
                    "the refusal must be the capability one, not {wrong:?}, got {message:?}"
                );
            }
        }
        other => panic!("expected an error, got {other:?}"),
    }
}

/// The gate runs before the field name is resolved, so an ungranted plugin is
/// never told whether a field exists -- assigning a name that is not a pal
/// field at all still reports the missing capability and nothing else. Order
/// the two the other way round and this is where it shows.
#[test]
fn an_ungranted_assignment_is_refused_before_the_field_name_is_resolved() {
    let mut harness = support::harness(&[Capability::SaveRead]);
    let (status, _) = harness.run(&first_pal("target.no_such_field = 4\nreturn 'unreachable'"));
    match status {
        RunStatus::Error(message) => {
            assert!(
                message.contains("requires the save.write capability"),
                "an ungranted write must be refused for the capability, got {message:?}"
            );
            assert!(
                !message.contains("unknown pal field"),
                "refusing an ungranted plugin by field name tells it which fields exist, \
                 got {message:?}"
            );
        }
        other => panic!("expected an error, got {other:?}"),
    }
}

/// Real keys of `active_skills.json` and `passive_skills.json`, spelled exactly
/// as those files spell them. An id outside its catalog is refused, so an
/// invented one would silently turn every success test below into an error
/// test.
const A_SKILL: &str = "EPalWazaID::FireBall";
const ANOTHER_SKILL: &str = "EPalWazaID::AcidRain";
const A_PASSIVE: &str = "Rare";

#[test]
fn a_list_field_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_pal(&format!(
        "target.active_skills = {{ '{A_SKILL}', '{ANOTHER_SKILL}' }}
         local out = target.active_skills
         return out[1] .. ',' .. out[2] .. ',' .. tostring(#out)"
    )));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some(format!("{A_SKILL},{ANOTHER_SKILL},2").as_str()));
}

#[test]
fn a_map_field_round_trips() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_pal(
        "target.work_suitability = { Handcraft = 3 }
         return tostring(target.work_suitability.Handcraft)",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("3"));
}

/// A dry run never flushes, so the value read back here cannot have come from
/// the save: it can only be the dirty DTO the write left in the cache. The
/// non-dry round-trips above would still pass if the read went all the way out
/// to the save and back, which is why this one exists.
#[test]
fn a_dry_run_reads_back_the_collections_it_just_set() {
    let mut harness = support::harness_dry(CAPS);
    let (status, summary) = harness.run(&first_pal(&format!(
        "target.active_skills = {{ '{A_SKILL}' }}
         target.work_suitability = {{ Mining = 7 }}
         return target.active_skills[1] .. '|' .. tostring(target.work_suitability.Mining)"
    )));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some(format!("{A_SKILL}|7").as_str()));
}

#[test]
fn a_read_returns_a_snapshot_not_a_live_view() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_pal(&format!(
        "target.active_skills = {{ '{A_SKILL}' }}
         table.insert(target.active_skills, '{ANOTHER_SKILL}')
         return tostring(#target.active_skills)"
    )));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("1"),
        "mutating the returned table must not write through; this pins the documented rule"
    );
}

#[test]
fn an_empty_table_clears_a_list_and_a_map() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_pal(&format!(
        "target.active_skills = {{ '{A_SKILL}' }}
         target.work_suitability = {{ Handcraft = 3 }}
         target.active_skills = {{}}
         target.work_suitability = {{}}
         local n = 0
         for _ in pairs(target.work_suitability) do n = n + 1 end
         return tostring(#target.active_skills) .. '|' .. tostring(n)"
    )));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("0|0"), "an empty table must clear both shapes");
}

#[test]
fn an_invalid_collection_entry_raises_and_names_the_entry() {
    let mut harness = support::harness(&[
        Capability::SaveRead,
        Capability::SaveWrite,
        Capability::GameData,
    ]);
    let (status, _) = harness.run(&first_pal(&format!(
        "target.active_skills = {{ '{A_SKILL}', 'NotARealSkill' }}
         return 'unreachable'"
    )));
    match status {
        RunStatus::Error(message) => assert!(
            message.contains("NotARealSkill"),
            "must name the offending entry, not just the field, got {message:?}"
        ),
        other => panic!("expected an error, got {other:?}"),
    }
}

/// `learned_skills` is picked from the *active* skill catalog in the app
/// (`LearnedSkillSelectModal` enumerates `activeSkillsData`), so a passive-skill
/// id must be refused there, and an active-skill id must be refused for
/// `passive_skills`. Without this, a single "is it any known skill" check would
/// pass both.
#[test]
fn each_skill_field_validates_against_its_own_catalog() {
    for (field, wrong_id, catalog) in [
        ("learned_skills", A_PASSIVE, "active_skills"),
        ("active_skills", A_PASSIVE, "active_skills"),
        ("passive_skills", A_SKILL, "passive_skills"),
    ] {
        let mut harness = support::harness(CAPS);
        let (status, _) = harness.run(&first_pal(&format!(
            "target.{field} = {{ '{wrong_id}' }}\nreturn 'unreachable'"
        )));
        match status {
            RunStatus::Error(message) => {
                assert!(message.contains(wrong_id), "{field} must name the entry, got {message:?}");
                assert!(
                    message.contains(catalog),
                    "{field} must name the catalog it checked, got {message:?}"
                );
            }
            other => panic!("expected {field} to reject {wrong_id}, got {other:?}"),
        }
    }
}

/// The same ids the app itself offers must be accepted, in every skill field.
#[test]
fn the_catalogs_own_ids_are_accepted_in_every_skill_field() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_pal(&format!(
        "target.learned_skills = {{ '{A_SKILL}' }}
         target.active_skills = {{ '{ANOTHER_SKILL}' }}
         target.passive_skills = {{ '{A_PASSIVE}' }}
         return target.learned_skills[1] .. '|' .. target.active_skills[1] .. '|' .. target.passive_skills[1]"
    )));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some(format!("{A_SKILL}|{ANOTHER_SKILL}|{A_PASSIVE}").as_str()));
}

#[test]
fn an_unknown_work_suitability_key_raises_and_names_the_key() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_pal(
        "target.work_suitability = { kindling = 3 }\nreturn 'unreachable'",
    ));
    match status {
        RunStatus::Error(message) => assert!(
            message.contains("kindling"),
            "must name the offending key, got {message:?}"
        ),
        other => panic!("expected an error, got {other:?}"),
    }
}

/// `work_suitability`'s key order is a wire contract (`psp-core/src/dto/pal.rs`
/// says so outright, and `OrderedMap` exists to keep it), and the order the
/// frontend receives is the order the entries sit in inside the save. Lua's
/// `pairs` has no order at all, so the map is built by walking
/// `WORK_SUITABILITIES` rather than the keys Lua hands over -- this reads the
/// order back out of the save itself, because a by-name lookup through the API
/// passes just as happily with the keys completely scrambled.
///
/// The three keys are chosen so their canonical order is neither the order they
/// are assigned in, nor their alphabetical order, nor the reverse of that:
/// canonically they run EmitFlame, Watering, Seeding, GenerateElectricity;
/// ascending they run EmitFlame, GenerateElectricity, Seeding, Watering; and
/// descending, Watering, Seeding, GenerateElectricity, EmitFlame. Three keys
/// were not enough: any three of these sit in canonical order or its exact
/// reverse, so a descending sort survived them. The fourth key is what
/// separates canonical order from every sort of it.
#[test]
fn work_suitability_reaches_the_save_in_the_canonical_key_order() {
    let mut harness = support::harness(CAPS_RAW);
    let (status, summary) = harness.run(
        "local id
         for p in save.pals() do id = p.instance_id break end
         local target
         for p in save.pals() do if p.instance_id == id then target = p break end end

         target.work_suitability = { GenerateElectricity = 3, Watering = 1, Seeding = 2, EmitFlame = 4 }
         local _ = target.level

         local count = raw.len('level', 'worldSaveData.CharacterSaveParameterMap')
         local index
         for i = 0, count - 1 do
           local this_id = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. i .. '].key.InstanceId')
           if this_id == id then index = i break end
         end
         assert(index ~= nil, 'pal entry not found in CharacterSaveParameterMap')
         local list = 'worldSaveData.CharacterSaveParameterMap[' .. index ..
             '].value.RawData.SaveParameter.GotWorkSuitabilityAddRankList'
         local n = raw.len('level', list)
         local parts = {}
         for i = 0, n - 1 do
           parts[#parts+1] = raw.get('level', list .. '[' .. i .. '].WorkSuitability')
         end
         return table.concat(parts, ',')",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some(
            "EPalWorkSuitability::EmitFlame,\
             EPalWorkSuitability::Watering,\
             EPalWorkSuitability::Seeding,\
             EPalWorkSuitability::GenerateElectricity"
        ),
        "the entries must sit in WORK_SUITABILITIES order -- not the order Lua enumerated them, \
         and not sorted"
    );
}

#[test]
fn assigning_a_non_table_to_a_collection_raises() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(&first_pal("target.active_skills = 5\nreturn 'unreachable'"));
    assert!(matches!(status, RunStatus::Error(_)));
}

#[test]
fn assigning_a_wrongly_shaped_table_to_a_collection_raises_and_names_the_field() {
    for (field, source) in [
        ("active_skills", "target.active_skills = { 1, 2 }"),
        ("active_skills", "target.active_skills = { Handcraft = 3 }"),
        ("work_suitability", "target.work_suitability = { 'Handcraft' }"),
        ("work_suitability", "target.work_suitability = { Handcraft = 'three' }"),
    ] {
        let mut harness = support::harness(CAPS);
        let (status, _) = harness.run(&first_pal(&format!("{source}\nreturn 'unreachable'")));
        match status {
            RunStatus::Error(message) => assert!(
                message.contains(field),
                "{source} must name the field it refused, got {message:?}"
            ),
            other => panic!("expected {source} to raise, got {other:?}"),
        }
    }
}

/// Reading a pal's collections and writing them straight back is the first
/// thing any editing plugin does, so the ids the save already carries have to
/// pass the same validation an assignment does. A wrong catalog for
/// `learned_skills`, or a casing mismatch between a save's ids and the
/// catalog's, would show up here and nowhere else -- the hand-written ids in
/// the tests above are catalog keys by construction. Each of the four fields
/// gets its own populated pal, because a pal carrying active skills usually
/// carries no learned ones and would leave that field untested.
#[test]
fn the_saves_own_collection_values_survive_a_round_trip() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local ids = {}
         for p in save.pals() do
           if #p.learned_skills > 0 and not ids.learned then ids.learned = p.instance_id end
           if #p.active_skills > 0 and not ids.active then ids.active = p.instance_id end
           if #p.passive_skills > 0 and not ids.passive then ids.passive = p.instance_id end
           local ranks = 0
           for _ in pairs(p.work_suitability) do ranks = ranks + 1 end
           if ranks > 0 and not ids.work then ids.work = p.instance_id end
           if ids.learned and ids.active and ids.passive and ids.work then break end
         end
         assert(ids.learned, 'the fixture must hold a pal with learned skills')
         assert(ids.active, 'the fixture must hold a pal with active skills')
         assert(ids.passive, 'the fixture must hold a pal with passive skills')
         assert(ids.work, 'the fixture must hold a pal with work-suitability ranks')

         local failures = {}
         for _, id in pairs(ids) do
           local target
           for p in save.pals() do if p.instance_id == id then target = p break end end
           local ok, err = pcall(function()
             target.learned_skills = target.learned_skills
             target.active_skills = target.active_skills
             target.passive_skills = target.passive_skills
             target.work_suitability = target.work_suitability
           end)
           if not ok then failures[#failures+1] = tostring(err) end
         end
         return #failures .. '|' .. table.concat(failures, ';')",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("0|"),
        "every value already in the save must be assignable again"
    );
}

/// The save cannot hold a zero rank: `apply_pal_dto` filters `rank != 0` out of
/// `GotWorkSuitabilityAddRankList` and drops the property outright when nothing
/// survives. If the cache kept the zero anyway, the same expression would answer
/// `0` before a flush and `nil` after one -- so this reads the key twice, once
/// straight after the write and once after forcing the flush, and requires the
/// two to agree.
#[test]
fn a_zero_rank_reads_the_same_before_and_after_the_flush() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(&first_pal(
        "target.work_suitability = { Mining = 0, Handcraft = 2 }
         local before = tostring(target.work_suitability.Mining)
         local before_n = 0
         for _ in pairs(target.work_suitability) do before_n = before_n + 1 end
         -- reading a field this run has not written rebuilds the pal snapshot,
         -- and that rebuild flushes the pending write out to the save first.
         local _ = target.level
         local after = tostring(target.work_suitability.Mining)
         local after_n = 0
         for _ in pairs(target.work_suitability) do after_n = after_n + 1 end
         return before .. '|' .. after .. '|' .. before_n .. '|' .. after_n",
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("nil|nil|1|1"),
        "a zero rank must be absent on both sides of the flush, and must not be counted"
    );
}

/// A dry run is a preview of the real run, so the two must agree about what a
/// zero rank does. Nothing flushes under a dry run, so if the cached DTO kept a
/// value the flush would have discarded, this is where the preview and the real
/// result part company.
#[test]
fn a_dry_run_and_a_real_run_agree_about_a_zero_rank() {
    let script = first_pal(
        "target.work_suitability = { Mining = 0, Handcraft = 2 }
         local n = 0
         for _ in pairs(target.work_suitability) do n = n + 1 end
         return tostring(target.work_suitability.Mining) .. '|' .. n",
    );

    let (dry_status, dry) = support::harness_dry(CAPS).run(&script);
    let (real_status, real) = support::harness(CAPS).run(&script);

    assert_eq!(dry_status, RunStatus::Ok);
    assert_eq!(real_status, RunStatus::Ok);
    assert_eq!(dry.as_deref(), Some("nil|1"), "the dry run must not preview a zero rank");
    assert_eq!(dry, real, "a dry run must report what the real run produces");
}

/// A mis-cased id is stored exactly as it was assigned -- nothing rewrites it to
/// the catalog's spelling, and `apply_pal_dto` writes the skill lists through
/// with no catalog check of its own -- so accepting one would put a spelling the
/// game does not know into the save. The catalogs are replaced here because the
/// checked-in `data/json` tree and the fixture save agree on casing, so nothing
/// in the real tree can exercise this.
#[test]
fn a_mis_cased_skill_id_is_refused_rather_than_stored_as_written() {
    let catalog = r#"{"EPalWazaID::FireBall": {"element": "Fire"}}"#;

    let mut exact = support::harness(CAPS).with_game_data_entries(&[("active_skills", catalog)]);
    let (status, summary) = exact.run(&first_pal(
        "target.active_skills = { 'EPalWazaID::FireBall' }
         return target.active_skills[1]",
    ));
    assert_eq!(status, RunStatus::Ok, "the catalog's own spelling must be accepted");
    assert_eq!(summary.as_deref(), Some("EPalWazaID::FireBall"));

    for wrong in ["epalwazaid::fireball", "EPALWAZAID::FIREBALL", "EPalWazaID::fireball"] {
        let mut harness = support::harness(CAPS).with_game_data_entries(&[("active_skills", catalog)]);
        let (status, _) = harness.run(&first_pal(&format!(
            "target.active_skills = {{ '{wrong}' }}\nreturn 'unreachable'"
        )));
        match status {
            RunStatus::Error(message) => assert!(
                message.contains(wrong),
                "must name the id it refused, got {message:?}"
            ),
            other => panic!("{wrong:?} differs from the catalog only in case and must be refused, got {other:?}"),
        }
    }
}

/// The `is_lucky` row declares its catalog check as a *pre*check, ahead of its
/// own `validate`, and that position is the whole reason the row model carries
/// two. This is the case that tells the two apart.
///
/// A `Boss_`-cased id passes `character_id_carries_boss_prefix`, which is
/// case-insensitive, but fails `boss_prefix_is_a_lucky_marker`, whose
/// `strip_prefix("BOSS_")` is not -- so `validate_is_lucky` refuses it on the
/// species name. With the catalog also unavailable, that answer is not merely
/// second-best, it is untrue: nothing here knows whether the prefix belongs to
/// the species, because the catalog that would say so did not load. The
/// catalog refusal has to win, and it only does while it runs first.
///
/// Move the row to `rw_postchecked` and every other pal test stays green while
/// this one reports the species answer instead.
#[test]
fn the_catalog_refusal_pre_empts_the_species_refusal_it_cannot_confirm() {
    let mut harness = support::harness(CAPS_RAW).with_empty_game_data();
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
            assert!(message.contains("is_lucky"), "must name the field, got {message:?}");
            assert!(
                message.contains("catalog is unavailable"),
                "the catalog refusal must win: without a catalog nothing can say whether the \
                 prefix belongs to the species, got {message:?}"
            );
            assert!(
                !message.contains("species name"),
                "asserting the prefix is part of the species name is exactly what an unavailable \
                 catalog cannot support, got {message:?}"
            );
        }
        other => panic!("expected an error, got {other:?}"),
    }
}
