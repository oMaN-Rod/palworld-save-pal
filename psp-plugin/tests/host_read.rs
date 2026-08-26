mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

fn read_harness() -> support::Harness {
    support::harness(&[Capability::SaveRead, Capability::GameData])
}

#[test]
fn pal_boss_and_lucky_flags_survive_a_raw_delete_that_shifts_positions() {
    let mut h = support::harness(&[Capability::SaveRead, Capability::SaveRaw]);
    let (status, value) = h.run(
        "local before, before_count = {}, 0
         for p in save.pals() do
           before[p.instance_id] = tostring(p.is_boss) .. '|' .. tostring(p.is_lucky)
           before_count = before_count + 1
         end

         local removed = raw.delete('level', 'worldSaveData.CharacterSaveParameterMap[0]')

         local checked, mismatches = 0, 0
         for p in save.pals() do
           local prev = before[p.instance_id]
           if prev ~= nil then
             checked = checked + 1
             local now = tostring(p.is_boss) .. '|' .. tostring(p.is_lucky)
             if now ~= prev then mismatches = mismatches + 1 end
           end
         end

         return tostring(removed) .. ',' .. before_count .. ',' .. checked .. ',' .. mismatches",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<&str> = value.split(',').collect();
    let [removed, before_count, checked, mismatches] = parts[..] else {
        panic!("expected four comma-separated values, got {value}");
    };
    assert_eq!(removed, "true", "the delete must actually remove an entry");
    let before_count: i64 = before_count.parse().expect("a number");
    let checked: i64 = checked.parse().expect("a number");
    assert!(before_count > 0);
    // Only if the deleted entry was itself a pal (not a player) may exactly one pal drop out of the surviving set.
    assert!(
        checked >= before_count - 1,
        "expected nearly every pal to survive the delete, got {checked} of {before_count}"
    );
    assert_eq!(mismatches, "0", "no surviving pal's flags may change from an unrelated deletion");
}

#[test]
fn save_info_reports_the_loaded_world() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local i = save.info()
         return i.world_name .. '|' .. tostring(i.player_count > 0) .. '|' .. tostring(i.guild_count >= 0)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string comes back");
    assert!(value.ends_with("|true|true"), "got {value}");
}

#[test]
fn players_iterate_and_expose_their_fields() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local n, named = 0, 0
         for p in save.players() do
           n = n + 1
           if type(p.uid) == 'string' and #p.uid == 36 then named = named + 1 end
         end
         return n .. ',' .. named",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let (count, named) = value.split_once(',').expect("two numbers");
    assert_eq!(count, named, "every player must expose a well-formed uid");
    assert!(count.parse::<i64>().expect("a number") > 0);
}

#[test]
fn players_expose_last_online_ts_consistently_with_last_online() {
    // The sandbox has no `os` library to parse the ISO string, so `last_online_ts` is what a script actually compares against.
    let mut h = read_harness();
    let (status, value) = h.run(
        "local n, ok, saw_both = 0, true, false
         for p in save.players() do
           n = n + 1
           local ts_is_nil = p.last_online_ts == nil
           local iso_is_nil = p.last_online == nil
           if ts_is_nil ~= iso_is_nil then ok = false end
           if not ts_is_nil then
             if type(p.last_online_ts) ~= 'number' or p.last_online_ts <= 0 then ok = false end
             saw_both = true
           end
         end
         return tostring(n > 0) .. ',' .. tostring(ok) .. ',' .. tostring(saw_both)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,true,true"));
}

#[test]
fn the_player_count_matches_save_info() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local n = 0 for _ in save.players() do n = n + 1 end
         return tostring(n == save.info().player_count)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true"));
}

#[test]
fn pals_iterate_and_derive_is_boss_excluding_lucky_pals() {
    // A lucky pal carries the same BOSS_ prefix as a boss but is never a boss, by construction in Palworld itself.
    let mut h = read_harness();
    let (status, value) = h.run(
        "local n, ok = 0, true
         for p in save.pals() do
           n = n + 1
           local prefixed = p.character_id:sub(1, 5):upper() == 'BOSS_'
           local expected = prefixed and not p.is_lucky
           if p.is_boss ~= expected then ok = false end
         end
         return tostring(n > 0) .. ',' .. tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,true"));
}

#[test]
fn pals_expose_is_lucky_and_it_never_coincides_with_is_boss() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local ok, saw_lucky, saw_not_lucky = true, false, false
         for p in save.pals() do
           if type(p.is_lucky) ~= 'boolean' then ok = false end
           if p.is_lucky then saw_lucky = true else saw_not_lucky = true end
           if p.is_boss and p.is_lucky then ok = false end
         end
         return tostring(ok) .. ',' .. tostring(saw_lucky) .. ',' .. tostring(saw_not_lucky)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,true,true"));
}

#[test]
fn pals_expose_nickname_owner_uid_and_talents() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local n, saw_owner, ok = 0, false, true
         for p in save.pals() do
           n = n + 1
           local nt = type(p.nickname)
           if nt ~= 'string' and nt ~= 'nil' then ok = false end
           local ot = type(p.owner_uid)
           if ot ~= 'string' and ot ~= 'nil' then ok = false end
           if ot == 'string' then
             if #p.owner_uid ~= 36 then ok = false end
             saw_owner = true
           end
           if type(p.talent_hp) ~= 'number' then ok = false end
           if type(p.talent_shot) ~= 'number' then ok = false end
           if type(p.talent_defense) ~= 'number' then ok = false end
         end
         return tostring(n > 0) .. ',' .. tostring(saw_owner) .. ',' .. tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,true,true"));
}

#[test]
fn guilds_iterate_and_expose_their_counts() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local n = 0
         for g in save.guilds() do
           n = n + 1
           assert(type(g.id) == 'string')
           assert(type(g.player_count) == 'number')
           assert(type(g.base_count) == 'number')
         end
         return tostring(n == save.info().guild_count)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true"));
}

#[test]
fn guilds_expose_a_well_formed_admin_uid_when_present() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local n, ok, saw_admin = 0, true, false
         for g in save.guilds() do
           n = n + 1
           local t = type(g.admin_uid)
           if t ~= 'string' and t ~= 'nil' then ok = false end
           if t == 'string' then
             if #g.admin_uid ~= 36 then ok = false end
             saw_admin = true
           end
         end
         return tostring(n > 0) .. ',' .. tostring(ok) .. ',' .. tostring(saw_admin)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,true,true"));
}

#[test]
fn bases_iterate_and_expose_a_well_formed_id_and_guild_id() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local n, well_formed = 0, 0
         for b in save.bases() do
           n = n + 1
           if type(b.id) == 'string' and #b.id == 36
              and type(b.guild_id) == 'string' and #b.guild_id == 36 then
             well_formed = well_formed + 1
           end
         end
         return n .. ',' .. well_formed",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let (n, well_formed) = value.split_once(',').expect("two numbers");
    assert_eq!(n, well_formed, "every base must expose a well-formed id and guild_id");
    assert!(n.parse::<i64>().expect("a number") > 0);
}

#[test]
fn bases_expose_their_world_coordinates() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local n, well_formed, nonzero = 0, 0, 0
         local seen, distinct = {}, 0
         for b in save.bases() do
           n = n + 1
           if type(b.x) == 'number' and type(b.y) == 'number' and type(b.z) == 'number' then
             well_formed = well_formed + 1
             if b.x ~= 0 or b.y ~= 0 or b.z ~= 0 then nonzero = nonzero + 1 end
             local key = string.format('%f/%f/%f', b.x, b.y, b.z)
             if not seen[key] then seen[key] = true distinct = distinct + 1 end
           end
         end
         return table.concat({ n, well_formed, nonzero, distinct }, ',')",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<i64> =
        value.split(',').map(|p| p.parse().expect("four numbers")).collect();
    let (n, well_formed, nonzero, distinct) = (parts[0], parts[1], parts[2], parts[3]);

    assert!(n > 0, "the fixture has bases");
    assert_eq!(n, well_formed, "every base must expose numeric x/y/z coordinates");
    assert_eq!(n, nonzero, "no base sits at the origin; all-zero means the lookup failed");
    assert_eq!(n, distinct, "every base has its own position");
}

#[test]
fn a_players_pals_are_a_subset_of_all_pals() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local total = 0 for _ in save.pals() do total = total + 1 end
         local owned = 0
         for p in save.players() do for _ in p.pals() do owned = owned + 1 end end
         return tostring(owned <= total) .. ',' .. tostring(owned > 0)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,true"));
}

#[test]
fn an_unknown_field_reads_as_nil_rather_than_erroring() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "for p in save.players() do return type(p.no_such_field) end return 'no players'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil"));
}

#[test]
fn a_handle_metatable_cannot_be_reached_from_lua() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "for p in save.players() do return type(getmetatable(p)) end return 'no players'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("string"));
}

#[test]
fn a_handle_from_one_kind_is_not_accepted_as_another() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local player for p in save.players() do player = p break end
         -- guild fields must not resolve against a player handle
         return tostring(player.player_count)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil"));
}

#[test]
fn gamedata_validates_items_and_pals() {
    // The catalogs key on INTERNAL ids, not display names: "CuteFox" is the id for the pal whose English name is "Foxparks".
    let mut h = read_harness();
    let (status, value) = h.run(
        "return tostring(gamedata.is_valid_item('Wood')) .. ',' ..
                tostring(gamedata.is_valid_item('DefinitelyNotAnItem')) .. ',' ..
                tostring(gamedata.is_valid_pal('CuteFox')) .. ',' ..
                tostring(gamedata.is_valid_pal('Alpaca')) .. ',' ..
                tostring(gamedata.is_valid_pal('DefinitelyNotAPal'))",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,false,true,true,false"));
}

#[test]
fn the_save_global_is_absent_without_the_read_capability() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run("return type(save) .. ',' .. type(gamedata)");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil,table"));
}

#[test]
fn the_gamedata_global_is_absent_without_its_capability() {
    let mut h = support::harness(&[Capability::SaveRead]);
    let (status, value) = h.run("return type(save) .. ',' .. type(gamedata)");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("table,nil"));
}

/// The budget is wall clock inside the sandbox, so it is charged for every
/// sibling test loading its own copy of the corpus on another core. Twenty
/// passes take about two seconds on an idle machine and comfortably over ten
/// under the rest of this package, which is why the ceiling is set where a
/// contended run still fits and a per-pal snapshot rebuild -- minutes, even
/// idle -- still does not.
#[test]
fn iteration_over_the_whole_corpus_stays_within_the_time_budget() {
    let mut h = support::harness_with_timeout(&[Capability::SaveRead], 60_000);
    let (status, value) = h.run(
        "local n = 0
         for _ = 1, 20 do for p in save.pals() do n = n + p.level end end
         return tostring(n > 0)",
    );
    assert_eq!(status, RunStatus::Ok, "twenty full passes must stay linear in the pal count");
    assert_eq!(value.as_deref(), Some("true"));
}

#[test]
fn every_read_function_survives_hostile_arguments() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local vals = { nil, true, 0, -1, 1/0, 0/0, '', 'x', {}, print }
         for _, fn in pairs({ save.info, save.players, save.pals, save.guilds, save.bases,
                              gamedata.is_valid_item, gamedata.is_valid_pal }) do
           for i = 1, 10 do pcall(fn, vals[i]) end
         end
         return 'survived'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("survived"));
}

#[test]
fn a_guild_exposes_a_chest_container_id_that_resolves() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local found, resolves = 0, 0
         for g in save.guilds() do
           local id = g.chest_container_id
           if id then
             found = found + 1
             for c in save.containers() do
               if c.id == id then resolves = resolves + 1 break end
             end
           end
         end
         return string.format('%d,%d', found, resolves)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<i64> = value.split(',').map(|p| p.parse().expect("an integer")).collect();
    assert!(parts[0] > 0, "the fixture must have a guild with a chest: {value}");
    assert_eq!(parts[0], parts[1], "every exposed chest id must resolve to a real container: {value}");
}

#[test]
fn map_objects_iterate_with_their_identity_and_attachment() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local total, attached, named, with_hp = 0, 0, 0, 0
         local first_kind = nil
         for obj in save.map_objects() do
           total = total + 1
           if obj.base_id ~= nil then attached = attached + 1 end
           if obj.id ~= nil and obj.id ~= '' then named = named + 1 end
           if obj.max_hp ~= nil and obj.max_hp > 0 then with_hp = with_hp + 1 end
           if first_kind == nil then first_kind = obj.kind end
           if obj.instance_id == nil or obj.instance_id == '' then
             error('every map object must be addressable')
           end
         end
         return table.concat({ total, attached, named, with_hp, tostring(first_kind ~= nil and first_kind ~= '') }, ',')",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<&str> = value.split(',').collect();
    let [total, attached, named, _with_hp, has_kind] = parts[..] else {
        panic!("expected five comma-separated values, got {value}");
    };
    assert_eq!(total, "5452", "the fixture's map object count");
    assert_eq!(attached, "2144", "map objects belonging to a base");
    assert_eq!(named, "5452", "MapObjectId is a Name on every entry");
    assert_eq!(has_kind, "true");
}

#[test]
fn an_unattached_map_object_reports_nil_rather_than_a_zero_guid() {
    let mut h = read_harness();
    let (status, value) = h.run(
        "local nil_guid = 0
         for obj in save.map_objects() do
           if obj.base_id == '00000000-0000-0000-0000-000000000000' then
             nil_guid = nil_guid + 1
           end
         end
         return tostring(nil_guid)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("0"));
}

/// `save.map_objects()` respects the same gate every other `save.*` function
/// does: without `save.read`, the `save` global itself is absent, so the call
/// fails the same way any other index into a nil `save` would.
#[test]
fn map_objects_require_the_read_capability() {
    let mut h = support::harness(&[Capability::GameData]);
    let (status, value) = h.run(
        "local ok = pcall(function() for _ in save.map_objects() do end end)
         return tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        value.as_deref(),
        Some("false"),
        "map_objects must be unreachable without save.read, exactly like every other save.* function"
    );
}
