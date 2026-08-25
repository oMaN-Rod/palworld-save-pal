use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use psp_core::domain::{containers, pal, player};
use psp_core::dto::container::{ItemContainerDto, ItemContainerSlotDto};
use psp_core::error::CoreError;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::props;
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};

use psp_app::plugin_registry::BUNDLED;
use psp_plugin::manifest::Manifest;
use psp_plugin::runtime::{run_command, RunOutcome, RunRequest, RunServices};
use psp_plugin::sandbox::{Cancel, Limits};
use psp_plugin::status::RunStatus;

use uuid::Uuid;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("psp-app has a parent directory")
        .to_path_buf()
}

fn fixture_dir() -> PathBuf {
    repo_root().join("tests/fixtures/saves/v1_relics")
}

fn collect_player_file_refs(dir: &Path) -> BTreeMap<Uuid, PlayerFileData> {
    let mut player_file_refs = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("Players")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("sav") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let (stem, is_dps) = match stem.strip_suffix("_dps") {
                Some(base) => (base, true),
                None => (stem, false),
            };
            let Ok(uid) = Uuid::parse_str(stem) else { continue };
            let slot = player_file_refs
                .entry(uid)
                .or_insert(PlayerFileData::Paths { sav: None, dps: None });
            if let PlayerFileData::Paths { sav, dps } = slot {
                if is_dps { *dps = Some(path) } else { *sav = Some(path) }
            }
        }
    }
    player_file_refs
}

fn load_fixture() -> SaveSession {
    let dir = fixture_dir();
    let level = std::fs::read(dir.join("Level.sav")).expect("the corpus fixture is checked in");
    reparse_with_dir(&dir, &level).expect("the corpus fixture must load; a failure here is a repo bug")
}

/// Re-parses `level_bytes` through the exact path `SaveSession::load` uses,
/// against the same fixture directory's `LevelMeta.sav`/`Players/*.sav` --
/// the shape test 11 needs to prove a command's output save is not corrupt.
fn reparse_with_dir(dir: &Path, level_bytes: &[u8]) -> Result<SaveSession, CoreError> {
    let meta = std::fs::read(dir.join("LevelMeta.sav")).ok();
    let player_file_refs = collect_player_file_refs(dir);
    SaveSession::load(
        SaveKind::Steam { level_path: dir.join("Level.sav") },
        dir.to_string_lossy().into_owned(),
        "steam",
        level_bytes,
        meta.as_deref(),
        None,
        player_file_refs,
        None,
        false,
        &null_progress(),
    )
}

fn reparse(level_bytes: &[u8]) -> Result<SaveSession, CoreError> {
    reparse_with_dir(&fixture_dir(), level_bytes)
}

fn assert_round_trips(session: &SaveSession) {
    let bytes = session.level_sav_bytes().expect("the level serializes");
    reparse(&bytes).unwrap_or_else(|e| panic!("the written save did not reparse: {e}"));
}

/// Counts occurrences of any of `keys` as a property NAME anywhere under
/// `session.world_properties()`, walking the raw property tree directly --
/// independent of `raw.visit`, which is what the command under test uses.
fn count_raw_keys(session: &SaveSession, keys: &[&str]) -> usize {
    let wanted: std::collections::BTreeSet<&str> = keys.iter().copied().collect();
    let mut count = 0;
    count_raw_keys_in_properties(
        session.world_properties().expect("world_properties must resolve"),
        &wanted,
        &mut count,
    );
    count
}

fn count_raw_keys_in_properties(
    properties: &psp_core::ue::Properties,
    wanted: &std::collections::BTreeSet<&str>,
    count: &mut usize,
) {
    for (key, value) in properties {
        if wanted.contains(key.1.as_str()) {
            *count += 1;
        }
        count_raw_keys_in_property(value, wanted, count);
    }
}

fn count_raw_keys_in_property(
    property: &psp_core::ue::Property,
    wanted: &std::collections::BTreeSet<&str>,
    count: &mut usize,
) {
    use psp_core::ue::{Property, ValueVec};
    match property {
        Property::Struct(sv) => {
            if let Some(properties) = game_struct_properties(sv) {
                count_raw_keys_in_properties(properties, wanted, count);
            }
        }
        Property::Map(entries) => {
            for entry in entries {
                count_raw_keys_in_property(&entry.key, wanted, count);
                count_raw_keys_in_property(&entry.value, wanted, count);
            }
        }
        Property::Array(ValueVec::Struct(structs)) | Property::Set(ValueVec::Struct(structs)) => {
            for struct_value in structs {
                if let Some(properties) = game_struct_properties(struct_value) {
                    count_raw_keys_in_properties(properties, wanted, count);
                }
            }
        }
        _ => {}
    }
}

/// `RawData` fields decode into a typed [`psp_core::ue::games::palworld::PalStruct`]
/// rather than a generic struct, but several of its variants still carry a nested
/// dynamic property bag (a live character's `object`, an egg's contained pal, a
/// hatching egg's pal) where fields like `SkinName` actually live.
fn game_struct_properties(sv: &psp_core::ue::StructValue) -> Option<&psp_core::ue::Properties> {
    use psp_core::ue::games::palworld::{PalDynamicItemType, PalMapConcreteModelVariant, PalStruct};
    use psp_core::ue::StructValue;
    match sv {
        StructValue::Struct(properties) => Some(properties),
        StructValue::Game(PalStruct::CharacterData(data)) => Some(&data.object),
        StructValue::Game(PalStruct::DynamicItem(item)) => match &item.item_type {
            PalDynamicItemType::Egg { object, .. } => Some(object),
            _ => None,
        },
        StructValue::Game(PalStruct::MapConcreteModel(model)) => match &model.model_data {
            PalMapConcreteModelVariant::HatchingEgg(hatching) => {
                Some(&hatching.hatched_character_save_parameter)
            }
            _ => None,
        },
        _ => None,
    }
}

fn load_game_data() -> GameData {
    GameData::load(&repo_root().join("data/json")).expect("game data is checked in")
}

/// One `pst.cleanup` command run against a fresh copy of the `v1_relics`
/// fixture. Each test builds its own `Harness` -- state never carries over
/// between tests, matching the pattern `psp-plugin`'s own test harness uses.
struct Harness {
    session: SaveSession,
    game_data: GameData,
    manifest: Manifest,
    sources: BTreeMap<String, String>,
}

impl Harness {
    fn new() -> Self {
        let plugin = &BUNDLED[0];
        let manifest = Manifest::parse(plugin.manifest)
            .expect("the bundled manifest must parse");
        let sources = plugin
            .sources
            .iter()
            .map(|(name, content)| (name.to_string(), content.to_string()))
            .collect();
        Self { session: load_fixture(), game_data: load_game_data(), manifest, sources }
    }

    fn run(&mut self, command_id: &str, args: serde_json::Value, dry_run: bool) -> RunOutcome {
        let granted = self.manifest.capabilities.clone();
        run_command(
            RunRequest {
                manifest: &self.manifest,
                sources: &self.sources,
                command_id,
                args: &args,
                dry_run,
                granted: &granted,
            },
            RunServices {
                session: &mut self.session,
                game_data: &self.game_data,
                progress: None,
                storage: &BTreeMap::new(),
                confirm: None,
                limits: Limits::default(),
                cancel: Cancel::new(),
            },
        )
    }
}

const ALL_COMMANDS: &[&str] = &[
    "delete_all_skins",
    "delete_duplicated_players",
    "delete_empty_guilds",
    "delete_imported_pals",
    "delete_inactive_bases",
    "delete_inactive_players",
    "delete_invalid_structure_map_objects",
    "delete_non_base_map_objects",
    "delete_unreferenced_data",
    "fix_all_negative_timestamps",
    "remove_invalid_items_from_save",
    "remove_invalid_pals_from_save",
    "remove_invalid_passives_from_save",
];

// --- guild membership helpers (test 2 and the dry-run gate both need these) ---

/// A guild's members that are BOTH non-admin AND resolvable through the write
/// API (i.e. present in `player_summaries`, meaning the fixture ships a
/// `.sav` for them). See the task report: several of this fixture's guild
/// members have no shipped `.sav` at all and can never be resolved this way,
/// which is why `manufacture_empty_guild` has to force the last bit directly.
fn resolvable_non_admin_members(session: &SaveSession, guild_id: Uuid) -> Vec<Uuid> {
    let admin = session.guild_summaries.get(&guild_id).and_then(|g| g.admin_player_uid);
    session
        .player_summaries
        .iter()
        .filter(|(uid, p)| p.guild_id == Some(guild_id) && Some(**uid) != admin)
        .map(|(uid, _)| *uid)
        .collect()
}

/// Deletes a guild's resolvable non-admin members for real, then forces the
/// guild's cached `player_count` the rest of the way to zero. Returns the
/// guild's id.
///
/// No guild in the fixture can reach `player_count == 0` through the write
/// API alone: `delete_player` refuses a guild's admin while the guild is
/// loaded, AND several guilds carry members with no shipped `.sav` file at
/// all, which `player::delete_player` can never resolve (it requires
/// `player_file_refs`). Both floors apply to every guild in this fixture --
/// see the task report. This deletes what the API genuinely can, then forces
/// the remainder, standing in for however a real save could reach a
/// zero-member guild (a stale row surviving a crash or a manual edit)
/// without needing a matching ghost `.sav` in the test corpus.
fn manufacture_empty_guild(h: &mut Harness) -> Uuid {
    let target_guild = h
        .session
        .guild_summary_order
        .iter()
        .copied()
        .max_by_key(|id| resolvable_non_admin_members(&h.session, *id).len())
        .expect("the fixture has guilds");
    let victims = resolvable_non_admin_members(&h.session, target_guild);
    assert!(
        !victims.is_empty(),
        "the fixture must have a guild with at least one resolvable non-admin member"
    );

    let progress = null_progress();
    for victim in victims {
        // `delete_player` requires the player already in `loaded_players`,
        // same as the plugin's own `player.delete()` (`save_write.rs`) does
        // by calling `get_player_details` first.
        player::get_player_details(&mut h.session, &h.game_data, victim, &progress)
            .expect("loading a guild's non-admin member must succeed")
            .expect("non-admin member must resolve");
        let deleted = player::delete_player(&mut h.session, &h.game_data, victim, &progress)
            .expect("deleting a guild's non-admin member must succeed");
        assert!(deleted, "non-admin member {victim} must not be refused");
    }

    h.session
        .guild_summaries
        .get_mut(&target_guild)
        .expect("the target guild has a summary")
        .player_count = 0;

    target_guild
}

// --- base helpers (delete_inactive_bases) ---

fn count_bases(session: &SaveSession) -> usize {
    session.base_camp_map().map(|entries| entries.len()).unwrap_or(0)
}

fn base_exists(session: &SaveSession, id: Uuid) -> bool {
    session.base_camp_map().unwrap_or(&[]).iter().any(|entry| props::as_uuid(&entry.key) == Some(id))
}

/// Clones the fixture's first base entry under a fresh id and gives the clone
/// a `group_id_belong_to` no player's `guild_id` resolves to -- a guild the
/// command can find but for which `save.players()` yields no member at all,
/// distinct from a guild whose members are merely inactive. Returns the new
/// base's id.
fn seed_base_with_no_visible_members(h: &mut Harness) -> Uuid {
    let entries = h.session.base_camp_map().expect("the fixture must ship at least one base");
    assert!(!entries.is_empty(), "the fixture must ship at least one base");
    let mut cloned = entries[0].clone();

    let new_base_id = Uuid::new_v4();
    cloned.key = props::guid_property(new_base_id);

    let known_guild_ids: std::collections::HashSet<Uuid> =
        h.session.player_summaries.values().filter_map(|p| p.guild_id).collect();
    let mut orphan_guild_id = Uuid::new_v4();
    while known_guild_ids.contains(&orphan_guild_id) {
        orphan_guild_id = Uuid::new_v4();
    }

    let value_properties =
        props::struct_props_mut(&mut cloned.value).expect("the cloned base entry has a value struct");
    let raw_data =
        props::get_mut(value_properties, &["RawData"]).expect("the cloned base entry has RawData");
    let psp_core::ue::Property::Struct(psp_core::ue::StructValue::Game(
        psp_core::ue::PalStruct::BaseCamp(camp),
    )) = raw_data
    else {
        panic!("the cloned base entry's RawData is not a BaseCamp");
    };
    camp.group_id_belong_to = props::uuid_to_guid(orphan_guild_id);

    psp_core::domain::world::base_camp_map_mut(&mut h.session.level)
        .expect("base camp map")
        .expect("base camp map")
        .push(cloned);

    assert!(
        base_exists(&h.session, new_base_id),
        "the seeded orphan base must be readable before the command runs"
    );
    new_base_id
}

// --- timestamp helpers (fix_all_negative_timestamps and the temporal delete_inactive_players tests) ---

fn world_now_tick(session: &SaveSession) -> i64 {
    let world_props = psp_core::domain::world::world_props(&session.level)
        .expect("worldSaveData must be present");
    props::get(world_props, &["GameTimeSaveData", "RealDateTimeTicks"])
        .and_then(props::as_i64)
        .expect("GameTimeSaveData.RealDateTimeTicks must be a scalar i64")
}

/// Writes `LastOnlineRealTime` directly onto a player's `CharacterSaveParameterMap`
/// entry -- the same field `psp-core`'s own `transfer::sync_timestamps` writes.
/// `raw.set` cannot do this itself: it only writes an ALREADY-PRESENT scalar,
/// and this key is absent from every entry in the unmodified fixture (see the
/// task report), so seeding it is test setup, done through the domain layer
/// directly rather than through the plugin API under test.
///
/// A brand-new property also needs a schema entry before `level_sav_bytes()`
/// can serialize it -- `uesave` only knows the shape of properties it either
/// read or was told about (see `psp-core/tests/schema_priming.rs`), and
/// `LastOnlineRealTime` is absent from `psp-core`'s own curated
/// `save_parameter_schemas()` list precisely because nothing in `psp-core`
/// ever creates it fresh; `transfer::sync_timestamps` only ever updates a
/// copy that already exists. `raw.set` in the ACTUAL command never hits this:
/// it only writes where `raw.exists` already holds, so this priming step is
/// test-only, standing in for a real save where a prior gameplay event (a
/// guild transfer, for one) already introduced the property.
fn seed_last_online_real_time(session: &mut SaveSession, uid: Uuid, value: i64) {
    props::ensure_schema(
        &mut session.level,
        format!("{}.LastOnlineRealTime", psp_core::domain::pal::LEVEL_SAVE_PARAMETER_PREFIX),
        psp_core::ue::PropertyTagPartial {
            id: None,
            data: psp_core::ue::PropertyTagDataPartial::Other(psp_core::ue::PropertyType::Int64Property),
        },
    );
    let entries =
        psp_core::domain::world::character_map_mut(&mut session.level).expect("character map");
    let entry = entries
        .iter_mut()
        .find(|entry| {
            psp_core::domain::world::entry_is_player(entry)
                && psp_core::domain::world::entry_player_uid(entry) == Some(uid)
        })
        .expect("the player has a character entry");
    let save_parameter = psp_core::domain::world::entry_save_parameter_mut(entry)
        .expect("the player entry has a SaveParameter bag");
    save_parameter.insert("LastOnlineRealTime", props::int64_property(value));
}

// --- duplicate-player helpers (delete_duplicated_players) ---

fn count_character_entries(session: &SaveSession) -> usize {
    session.character_map().expect("character map must resolve").len()
}

fn character_entry_exists(session: &SaveSession, instance_id: Uuid) -> bool {
    session
        .character_map()
        .expect("character map must resolve")
        .iter()
        .any(|entry| psp_core::domain::world::entry_instance_id(entry) == Some(instance_id))
}

fn count_character_entries_for_uid(session: &SaveSession, uid: Uuid) -> usize {
    session
        .character_map()
        .expect("character map must resolve")
        .iter()
        .filter(|entry| {
            psp_core::domain::world::entry_is_player(entry)
                && psp_core::domain::world::entry_player_uid(entry) == Some(uid)
        })
        .count()
}

/// Clones the first fixture player's `CharacterSaveParameterMap` entry under a
/// fresh `InstanceId`, keeping the same `PlayerUId`, and gives the original
/// entry a more recent `LastOnlineRealTime` than the clone. Returns
/// `(uid, kept_instance, stale_instance)`.
fn seed_duplicate_player(session: &mut SaveSession) -> (Uuid, Uuid, Uuid) {
    let world_tick = world_now_tick(session);
    let uid = session.player_summary_order[0];

    let kept_instance = session
        .character_map()
        .expect("character map must resolve")
        .iter()
        .find(|entry| {
            psp_core::domain::world::entry_is_player(entry)
                && psp_core::domain::world::entry_player_uid(entry) == Some(uid)
        })
        .and_then(psp_core::domain::world::entry_instance_id)
        .expect("the target player has a character entry");

    seed_last_online_real_time(session, uid, world_tick - 1_000);

    let mut cloned_entry = session
        .character_map()
        .expect("character map must resolve")
        .iter()
        .find(|entry| psp_core::domain::world::entry_instance_id(entry) == Some(kept_instance))
        .cloned()
        .expect("the kept entry must still resolve");

    let stale_instance = Uuid::new_v4();
    if let Some(key_props) = props::struct_props_mut(&mut cloned_entry.key) {
        key_props.insert("InstanceId", props::guid_property(stale_instance));
    }
    if let Some(save_parameter) = psp_core::domain::world::entry_save_parameter_mut(&mut cloned_entry) {
        save_parameter.insert("LastOnlineRealTime", props::int64_property(world_tick - 1_000_000));
    }

    psp_core::domain::world::character_map_mut(&mut session.level)
        .expect("character map")
        .push(cloned_entry);

    assert!(
        character_entry_exists(session, stale_instance),
        "the seeded duplicate must be readable before the command runs"
    );
    assert_eq!(
        count_character_entries_for_uid(session, uid),
        2,
        "the seed must produce exactly two character entries for the duplicated uid"
    );

    (uid, kept_instance, stale_instance)
}

fn read_last_online_real_time(session: &SaveSession, uid: Uuid) -> Option<i64> {
    let entries = session.character_map().ok()?;
    let entry = entries.iter().find(|entry| {
        psp_core::domain::world::entry_is_player(entry)
            && psp_core::domain::world::entry_player_uid(entry) == Some(uid)
    })?;
    let save_parameter = psp_core::domain::world::entry_save_parameter(entry)?;
    props::get(save_parameter, &["LastOnlineRealTime"]).and_then(props::as_i64)
}

fn elapsed_days_since_last_online(session: &SaveSession) -> i64 {
    let now = chrono::Utc::now();
    session
        .player_summaries
        .values()
        .filter_map(|p| p.last_online_time.map(|t| (now - t.0.and_utc()).num_days()))
        .max()
        .expect("the fixture must have players with a readable last_online timestamp (see test 6a)")
}

// --- invalid-item helpers (remove_invalid_items_from_save) ---

const BOGUS_ITEM_ID: &str = "PSP_Test_Definitely_Bogus_Item";

/// Seeds a bogus item into the first fixture player's common inventory
/// container's slot 0, returning that container's id. The fixture ships no
/// bogus item of its own, so `remove_invalid_items_from_save`'s tests must
/// seed one -- see the task report.
fn seed_bogus_item(h: &mut Harness) -> (Uuid, i32) {
    let progress = null_progress();
    let target_uid = h.session.player_summary_order[0];
    let details = player::get_player_details(&mut h.session, &h.game_data, target_uid, &progress)
        .expect("player details load")
        .expect("the first fixture player exists");
    let container_id = details
        .common_container
        .expect("the player has a common inventory container")
        .id;

    // Two separate ways a seeded stack ends up invisible to the very command
    // being tested, both of which this picks its way around:
    //
    // 1. A NEW slot index is not safe. This fixture's slot 0 already exists in
    //    the raw array carrying a dangling dynamic-item reference, and
    //    `read_item_container` drops such a slot on the way out — so seeding
    //    there wrote a stack no read could ever return.
    // 2. An EXISTING slot that owns a dynamic item is not safe either. The
    //    incoming DTO has `dynamic_item: None`, so `apply_item_container_dto`'s
    //    cleanup pass deletes that dynamic entry while the raw slot keeps
    //    pointing at it — turning the slot into case 1.
    //
    // What is left is an existing slot holding a plain stackable (nil dynamic
    // id), which survives the overwrite and stays readable through the same
    // path the command uses.
    let slot_index = containers::read_item_container(
        &h.session.level,
        &mut h.session.caches,
        &h.game_data,
        container_id,
        "",
        None,
    )
    .expect("the container resolves")
    .slots
    .iter()
    .find(|slot| {
        slot.dynamic_item.is_none()
            && matches!(slot.local_id, None | Some(psp_core::props::EMPTY_UUID))
    })
    .expect("the first fixture player's inventory holds at least one plain stackable")
    .slot_index;

    let seed = ItemContainerDto {
        id: container_id,
        r#type: String::new(),
        slots: vec![ItemContainerSlotDto {
            dynamic_item: None,
            slot_index,
            count: 1,
            static_id: Some(BOGUS_ITEM_ID.to_string()),
            local_id: None,
        }],
        key: None,
        slot_num: 0,
    };
    containers::apply_item_container_dto(&mut h.session, container_id, &seed, None)
        .expect("seeding the bogus item must succeed");

    let seeded = containers::read_item_container(
        &h.session.level,
        &mut h.session.caches,
        &h.game_data,
        container_id,
        "",
        None,
    )
    .expect("the container still resolves")
    .slots
    .iter()
    .any(|slot| slot.slot_index == slot_index && slot.static_id.as_deref() == Some(BOGUS_ITEM_ID));
    assert!(
        seeded,
        "the bogus stack must be readable after seeding, or the tests built on it prove nothing"
    );

    (container_id, slot_index)
}

// --- pal helpers (delete_imported_pals, remove_invalid_pals_from_save) ---

/// One world pal's fields relevant to these two commands, read directly off
/// `CharacterSaveParameterMap` rather than through `pal::pal_summaries` or any
/// other function the commands under test also call.
struct WorldPalFields {
    is_imported: bool,
    is_boss: bool,
}

fn world_pal_fields(session: &SaveSession) -> Vec<WorldPalFields> {
    let entries = session.character_map().expect("character map must resolve");
    entries
        .iter()
        .filter(|entry| !psp_core::domain::world::entry_is_player(entry))
        .filter_map(|entry| {
            let save_parameter = psp_core::domain::world::entry_save_parameter(entry)?;
            let character_id =
                props::get(save_parameter, &["CharacterID"]).and_then(props::as_str).unwrap_or("");
            let is_imported = props::get(save_parameter, &["bImportedCharacter"])
                .and_then(props::as_bool)
                .unwrap_or(false);
            let is_boss = character_id.to_uppercase().starts_with("BOSS_");
            Some(WorldPalFields { is_imported, is_boss })
        })
        .collect()
}

fn count_pals_where(session: &SaveSession, predicate: impl Fn(&WorldPalFields) -> bool) -> usize {
    world_pal_fields(session).iter().filter(|p| predicate(p)).count()
}

/// Adds a brand-new, owner-resolvable world pal with the given species id, in
/// the first fixture player's pal box. A command that finds and deletes
/// exactly this one pal must bring the world pal count back to what it was
/// before this call -- which an in-place rewrite of an existing pal's
/// `CharacterID` cannot give a test to check against, since that pal's own
/// prior species is lost the moment it is overwritten.
fn seed_pal_with_character_id(h: &mut Harness, character_id: &str) {
    let progress = null_progress();
    let target_uid = h.session.player_summary_order[0];
    let details = player::get_player_details(&mut h.session, &h.game_data, target_uid, &progress)
        .expect("player details load")
        .expect("the first fixture player exists");
    let pal_box_id = details.pal_box_id.expect("the player has a pal box container");
    pal::add_player_pal(&mut h.session, &h.game_data, target_uid, character_id, "psp seed", pal_box_id, None)
        .expect("adding the seeded pal must succeed")
        .expect("the pal box must have room for the seeded pal");
}

/// Sets `bImportedCharacter` on the first world pal entry that does not
/// already carry it, for a fixture that ships no imported pal of its own.
fn seed_imported_pal(session: &mut SaveSession) {
    let entries =
        psp_core::domain::world::character_map_mut(&mut session.level).expect("character map");
    let entry = entries
        .iter_mut()
        .find(|entry| {
            !psp_core::domain::world::entry_is_player(entry)
                && psp_core::domain::world::entry_save_parameter(entry).is_some_and(|save_parameter| {
                    !props::get(save_parameter, &["bImportedCharacter"])
                        .and_then(props::as_bool)
                        .unwrap_or(false)
                })
        })
        .expect("the fixture must have at least one non-imported world pal entry to mutate");
    let save_parameter = psp_core::domain::world::entry_save_parameter_mut(entry)
        .expect("the pal entry has a SaveParameter bag");
    save_parameter.insert("bImportedCharacter", props::bool_property(true));
}

// --- dps helpers (delete_imported_pals, remove_invalid_pals_from_save) ---

/// The uid of the one `v1_relics` fixture player who ships a `_dps.sav`, found
/// from the fixture directory itself rather than from anything a command's
/// own DPS walk resolves.
fn dps_player_uid() -> Uuid {
    collect_player_file_refs(&fixture_dir())
        .into_iter()
        .find_map(|(uid, refs)| match refs {
            PlayerFileData::Paths { dps: Some(_), .. } => Some(uid),
            _ => None,
        })
        .expect("the fixture must have at least one player with a DPS save")
}

fn dps_slot_count(dps_bytes: &[u8]) -> usize {
    let save = psp_core::savio::read_sav_bytes(dps_bytes).expect("the dps bytes parse");
    props::get(&save.root.properties, &["SaveParameterArray"])
        .and_then(props::struct_values)
        .map(|values| values.len())
        .unwrap_or(0)
}

fn dps_slot_character_id(dps_bytes: &[u8], slot_index: i32) -> Option<String> {
    let save = psp_core::savio::read_sav_bytes(dps_bytes).expect("the dps bytes parse");
    let array = props::get(&save.root.properties, &["SaveParameterArray"]).and_then(props::struct_values)?;
    let psp_core::ue::StructValue::Struct(slot_props) = array.get(slot_index as usize)? else {
        return None;
    };
    let save_parameter = slot_props
        .0
        .get(&psp_core::ue::PropertyKey::from("SaveParameter"))
        .and_then(props::struct_props)?;
    props::get(save_parameter, &["CharacterID"]).and_then(props::as_str).map(str::to_string)
}

fn dps_bytes_for(h: &Harness, uid: Uuid) -> Vec<u8> {
    h.session
        .player_sav_bytes()
        .expect("player_sav_bytes must resolve")
        .get(&uid)
        .and_then(|(_, dps)| dps.clone())
        .expect("the player must have dps bytes")
}

/// Adds a real pal into the dps-owning fixture player's dimensional storage and
/// flags it imported, returning its slot index. `add_player_dps_pal` picks the
/// first empty slot itself, so no index bookkeeping is needed to find one.
fn seed_dps_imported_pal(h: &mut Harness, uid: Uuid) -> i32 {
    let progress = null_progress();
    player::get_player_details(&mut h.session, &h.game_data, uid, &progress)
        .expect("player details load")
        .expect("the dps-owning fixture player exists");

    let (slot_index, _dto) =
        pal::add_player_dps_pal(&mut h.session, &h.game_data, uid, "Lamball", "psp seed", None)
            .expect("adding a dps pal must succeed")
            .expect("the player's dimensional storage must have an empty slot");

    let loaded = h.session.loaded_players.get_mut(&uid).expect("player is loaded");
    let dps_save = loaded.dps.as_mut().expect("player has a dps save");
    props::ensure_schema(
        dps_save,
        format!("{}.bImportedCharacter", pal::SLOT_SAVE_PARAMETER_PREFIX),
        psp_core::ue::PropertyTagPartial {
            id: None,
            data: psp_core::ue::PropertyTagDataPartial::Other(psp_core::ue::PropertyType::BoolProperty),
        },
    );
    let array = props::get_mut(&mut dps_save.root.properties, &["SaveParameterArray"])
        .and_then(props::struct_values_mut)
        .expect("SaveParameterArray present");
    let psp_core::ue::StructValue::Struct(slot_props) = &mut array[slot_index as usize] else {
        panic!("dps slot is not a struct");
    };
    let save_parameter = slot_props
        .0
        .get_mut(&psp_core::ue::PropertyKey::from("SaveParameter"))
        .and_then(props::struct_props_mut)
        .expect("SaveParameter present");
    save_parameter.insert("bImportedCharacter", props::bool_property(true));

    slot_index
}

/// Adds a pal with a catalog-unknown species id into the dps-owning fixture
/// player's dimensional storage, returning its slot index.
fn seed_dps_invalid_pal(h: &mut Harness, uid: Uuid) -> i32 {
    let progress = null_progress();
    player::get_player_details(&mut h.session, &h.game_data, uid, &progress)
        .expect("player details load")
        .expect("the dps-owning fixture player exists");

    let (slot_index, _dto) = pal::add_player_dps_pal(
        &mut h.session,
        &h.game_data,
        uid,
        "PSP_NOT_A_REAL_PAL",
        "psp seed",
        None,
    )
    .expect("adding a dps pal must succeed")
    .expect("the player's dimensional storage must have an empty slot");

    slot_index
}

// --- passive skill helpers (remove_invalid_passives_from_save) ---

const BOGUS_PASSIVE_ID: &str = "PSP_NOT_A_REAL_PASSIVE";

/// Counts every passive-skill entry across every world pal's `PassiveSkillList`,
/// read directly off `CharacterSaveParameterMap` rather than through
/// `pal.passive_skills` or anything else the command under test calls.
fn count_passive_entries(session: &SaveSession) -> usize {
    let entries = session.character_map().expect("character map must resolve");
    entries
        .iter()
        .filter(|entry| !psp_core::domain::world::entry_is_player(entry))
        .filter_map(psp_core::domain::world::entry_save_parameter)
        .filter_map(|save_parameter| props::get(save_parameter, &["PassiveSkillList"]))
        .filter_map(props::name_values)
        .map(|values| values.len())
        .sum()
}

/// The passive skills of the world pal with the given instance id, read the
/// same independent way `count_passive_entries` does.
fn passives_of(session: &SaveSession, target: Uuid) -> Vec<String> {
    let entries = session.character_map().expect("character map must resolve");
    entries
        .iter()
        .find(|entry| {
            !psp_core::domain::world::entry_is_player(entry)
                && psp_core::domain::world::entry_instance_id(entry) == Some(target)
        })
        .and_then(psp_core::domain::world::entry_save_parameter)
        .and_then(|save_parameter| props::get(save_parameter, &["PassiveSkillList"]))
        .and_then(props::name_values)
        .cloned()
        .unwrap_or_default()
}

/// Appends `skill_id` onto the first world pal that already carries at least
/// one passive skill, so the seeded skill lands alongside real ones on the
/// same pal, and returns that pal's instance id. The fixture ships no unknown
/// passive of its own, so a test that did not seed would assert zero.
fn seed_passive_skill(session: &mut SaveSession, skill_id: &str) -> Uuid {
    let entries =
        psp_core::domain::world::character_map_mut(&mut session.level).expect("character map");
    let entry = entries
        .iter_mut()
        .find(|entry| {
            !psp_core::domain::world::entry_is_player(entry)
                && psp_core::domain::world::entry_save_parameter(entry).is_some_and(|save_parameter| {
                    props::get(save_parameter, &["PassiveSkillList"])
                        .and_then(props::name_values)
                        .is_some_and(|values| !values.is_empty())
                })
        })
        .expect("the fixture must have at least one world pal carrying passive skills");
    let instance_id =
        psp_core::domain::world::entry_instance_id(entry).expect("pal entry has an instance id");
    let save_parameter = psp_core::domain::world::entry_save_parameter_mut(entry)
        .expect("the pal entry has a SaveParameter bag");
    let mut skills = props::get(save_parameter, &["PassiveSkillList"])
        .and_then(props::name_values)
        .cloned()
        .unwrap_or_default();
    skills.push(skill_id.to_string());
    save_parameter.insert("PassiveSkillList", props::name_array_property(skills));
    instance_id
}

/// One key from the loaded `passive_skills` catalog, for seeding a real
/// passive alongside a bogus one.
fn known_passive_skill(game_data: &GameData) -> String {
    game_data
        .get("passive_skills")
        .and_then(serde_json::Value::as_object)
        .and_then(|catalog| catalog.keys().next())
        .expect("the passive_skills catalog has at least one entry")
        .clone()
}

fn dps_slot_passive_skills(dps_bytes: &[u8], slot_index: i32) -> Vec<String> {
    let save = psp_core::savio::read_sav_bytes(dps_bytes).expect("the dps bytes parse");
    let array = props::get(&save.root.properties, &["SaveParameterArray"])
        .and_then(props::struct_values)
        .expect("SaveParameterArray present");
    let psp_core::ue::StructValue::Struct(slot_props) =
        array.get(slot_index as usize).expect("the slot is present")
    else {
        panic!("dps slot is not a struct");
    };
    let save_parameter = slot_props
        .0
        .get(&psp_core::ue::PropertyKey::from("SaveParameter"))
        .and_then(props::struct_props)
        .expect("SaveParameter present");
    props::get(save_parameter, &["PassiveSkillList"])
        .and_then(props::name_values)
        .cloned()
        .unwrap_or_default()
}

/// Adds a real dps pal and overwrites its `PassiveSkillList` with one known
/// catalog skill plus one bogus skill, returning the slot index and the
/// known skill's id.
fn seed_dps_passive_skill(h: &mut Harness, uid: Uuid) -> (i32, String) {
    let progress = null_progress();
    player::get_player_details(&mut h.session, &h.game_data, uid, &progress)
        .expect("player details load")
        .expect("the dps-owning fixture player exists");

    let (slot_index, _dto) =
        pal::add_player_dps_pal(&mut h.session, &h.game_data, uid, "Lamball", "psp seed", None)
            .expect("adding a dps pal must succeed")
            .expect("the player's dimensional storage must have an empty slot");

    let valid_skill = known_passive_skill(&h.game_data);

    let loaded = h.session.loaded_players.get_mut(&uid).expect("player is loaded");
    let dps_save = loaded.dps.as_mut().expect("player has a dps save");
    let array = props::get_mut(&mut dps_save.root.properties, &["SaveParameterArray"])
        .and_then(props::struct_values_mut)
        .expect("SaveParameterArray present");
    let psp_core::ue::StructValue::Struct(slot_props) = &mut array[slot_index as usize] else {
        panic!("dps slot is not a struct");
    };
    let save_parameter = slot_props
        .0
        .get_mut(&psp_core::ue::PropertyKey::from("SaveParameter"))
        .and_then(props::struct_props_mut)
        .expect("SaveParameter present");
    save_parameter.insert(
        "PassiveSkillList",
        props::name_array_property(vec![valid_skill.clone(), BOGUS_PASSIVE_ID.to_string()]),
    );

    (slot_index, valid_skill)
}

// --- map object / work helpers (delete_non_base_map_objects, delete_invalid_structure_map_objects) ---

fn count_map_objects(session: &SaveSession) -> usize {
    psp_core::domain::map_object::map_object_ids(session).expect("map object ids").len()
}

fn count_map_objects_where(
    session: &SaveSession,
    predicate: impl Fn(&psp_core::domain::map_object::MapObjectView) -> bool,
) -> usize {
    psp_core::domain::map_object::map_object_views(session)
        .expect("map object views")
        .iter()
        .filter(|v| predicate(v))
        .count()
}

fn any_map_object_belongs_to(session: &SaveSession, base_id: Uuid) -> bool {
    psp_core::domain::map_object::map_object_views(session)
        .expect("map object views")
        .iter()
        .any(|v| v.base_id == Some(base_id))
}

fn count_work_entries(session: &SaveSession) -> usize {
    psp_core::domain::world::work_values(&session.level)
        .expect("work values")
        .map(|values| values.len())
        .unwrap_or(0)
}

/// A `WorkSaveData` element's `RawData.base_data.owner_map_object_model_id`,
/// read directly off the raw property tree rather than through
/// `psp_core::domain::map_object`, which is what the command under test calls.
fn work_owner_uuid(value: &psp_core::ue::StructValue) -> Option<Uuid> {
    let psp_core::ue::StructValue::Struct(properties) = value else { return None };
    match properties.0.get(&psp_core::ue::PropertyKey::from("RawData"))? {
        psp_core::ue::Property::Struct(psp_core::ue::StructValue::Game(
            psp_core::ue::PalStruct::Work(work),
        )) => work.base_data.as_ref().map(|base| props::guid_to_uuid(&base.owner_map_object_model_id)),
        _ => None,
    }
}

/// Recomputes, from `WorkSaveData` and the session's current map objects, how
/// many work entries point at a map object id that no longer exists. Never
/// calls `remove_orphaned_works` -- this is the independent gate on it.
fn count_orphaned_works(session: &SaveSession) -> usize {
    let surviving: std::collections::HashSet<Uuid> =
        psp_core::domain::map_object::map_object_ids(session).expect("map object ids").into_iter().collect();
    psp_core::domain::world::work_values(&session.level)
        .expect("work values")
        .map(|values| {
            values
                .iter()
                .filter(|work| match work_owner_uuid(work) {
                    Some(owner) => !surviving.contains(&owner),
                    None => false,
                })
                .count()
        })
        .unwrap_or(0)
}

/// Removes the fixture's most-populated base -- so the seed is never vacuous
/// -- from `BaseCampSaveData`, and returns its id plus how many map objects
/// pointed at it beforehand.
fn orphan_one_base(session: &mut SaveSession) -> (Uuid, usize) {
    let base_ids: Vec<Uuid> = session
        .base_camp_map()
        .expect("the fixture must ship at least one base")
        .iter()
        .filter_map(|entry| props::as_uuid(&entry.key))
        .collect();
    assert!(!base_ids.is_empty(), "the fixture must ship at least one base");

    let target = base_ids
        .into_iter()
        .max_by_key(|id| count_map_objects_where(session, |v| v.base_id == Some(*id)))
        .expect("the fixture has at least one base");
    let expected = count_map_objects_where(session, |v| v.base_id == Some(target));

    psp_core::domain::world::base_camp_map_mut(&mut session.level)
        .expect("base camp map")
        .expect("base camp map")
        .retain(|entry| props::as_uuid(&entry.key) != Some(target));

    (target, expected)
}

/// Overwrites the first fixture map object's `MapObjectId` name property --
/// `psp_core::props` reaches it directly, since it lives in the outer
/// property bag rather than inside the typed `RawData` model.
fn seed_map_object_id(session: &mut SaveSession, name: &str) {
    let objects = psp_core::domain::world::map_object_values_mut(&mut session.level)
        .expect("map object values")
        .expect("the fixture must ship map objects");
    let object = objects.first_mut().expect("the fixture has map objects");
    let psp_core::ue::StructValue::Struct(properties) = object else {
        panic!("map object is not a struct");
    };
    properties.insert("MapObjectId", props::name_property(name));
}

// --- ownerless-pal / structure-reference helpers (delete_unreferenced_data) ---

/// One world pal's ownership fields, read directly off
/// `CharacterSaveParameterMap` and `BaseCampSaveData` rather than through
/// `pal::pal_summaries` or any function `delete_unreferenced_data` itself calls.
struct WorldPalOwnership {
    owner_uid: Option<Uuid>,
    base_id: Option<Uuid>,
}

fn base_worker_container_map(session: &SaveSession) -> BTreeMap<Uuid, Uuid> {
    let mut map = BTreeMap::new();
    let Some(entries) = session.base_camp_map() else { return map };
    for entry in entries {
        let Some(base_id) = props::as_uuid(&entry.key) else { continue };
        if let Some((_, container_id)) = psp_core::domain::guild::base_guild_and_container(entry) {
            map.insert(container_id, base_id);
        }
    }
    map
}

fn world_pal_ownership(session: &SaveSession) -> Vec<WorldPalOwnership> {
    let containers = base_worker_container_map(session);
    let entries = session.character_map().expect("character map must resolve");
    entries
        .iter()
        .filter(|entry| !psp_core::domain::world::entry_is_player(entry))
        .filter_map(|entry| {
            let save_parameter = psp_core::domain::world::entry_save_parameter(entry)?;
            let owner_uid = props::get(save_parameter, &["OwnerPlayerUId"]).and_then(props::as_uuid);
            let base_id = props::get(save_parameter, &["SlotId", "ContainerId", "ID"])
                .and_then(props::as_uuid)
                .and_then(|container_id| containers.get(&container_id).copied());
            Some(WorldPalOwnership { owner_uid, base_id })
        })
        .collect()
}

/// A pal counts as ownerless here exactly when `delete_unreferenced_data`'s
/// own predicate would select it: no base worker container claims it, and
/// its `OwnerPlayerUId` (present and non-nil) names no player this save
/// still has a record for.
fn count_ownerless_pals(session: &SaveSession) -> usize {
    let known_players: std::collections::HashSet<Uuid> =
        session.player_summaries.keys().copied().collect();
    world_pal_ownership(session)
        .iter()
        .filter(|p| {
            if p.base_id.is_some() {
                return false;
            }
            match p.owner_uid {
                None => false,
                Some(uid) if uid.is_nil() => false,
                Some(uid) => !known_players.contains(&uid),
            }
        })
        .count()
}

fn count_stale_map_object_builders(session: &SaveSession) -> usize {
    let known_players: std::collections::HashSet<Uuid> =
        session.player_summaries.keys().copied().collect();
    psp_core::domain::map_object::map_object_views(session)
        .expect("map object views")
        .iter()
        .filter(|v| v.build_player_uid.is_some_and(|uid| !known_players.contains(&uid)))
        .count()
}

fn map_object_builder(session: &SaveSession, id: Uuid) -> Option<Uuid> {
    psp_core::domain::map_object::read_map_object(session, id).and_then(|v| v.build_player_uid)
}

/// Overwrites the first built structure's `build_player_uid` with a uuid that
/// names no player in the fixture, returning the object's id and that ghost
/// uid.
fn seed_map_object_built_by_missing_player(session: &mut SaveSession) -> (Uuid, Uuid) {
    let target = psp_core::domain::map_object::map_object_views(session)
        .expect("map object views")
        .into_iter()
        .find(|v| v.build_player_uid.is_some())
        .expect("the fixture must carry a built structure");
    let ghost_uid = Uuid::new_v4();
    let wrote =
        psp_core::domain::map_object::set_map_object_builder(session, target.instance_id, Some(ghost_uid))
            .expect("write");
    assert!(wrote, "the target object must resolve");
    (target.instance_id, ghost_uid)
}

// --- dynamic-item helpers (delete_unreferenced_data) ---

fn dynamic_item_ids(session: &SaveSession) -> std::collections::HashSet<Uuid> {
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};
    psp_core::domain::world::dynamic_item_values(&session.level)
        .expect("dynamic item values")
        .iter()
        .filter_map(|value| {
            let StructValue::Struct(item_props) = value else { return None };
            match item_props.0.get(&PropertyKey::from("RawData"))? {
                Property::Struct(StructValue::Game(PalStruct::DynamicItem(item))) => {
                    Some(props::guid_to_uuid(&item.id.local_id_in_created_world))
                }
                _ => None,
            }
        })
        .collect()
}

fn count_dynamic_items(session: &SaveSession) -> usize {
    dynamic_item_ids(session).len()
}

fn dynamic_item_exists(session: &SaveSession, id: Uuid) -> bool {
    dynamic_item_ids(session).contains(&id)
}

/// Every dynamic item id an item-container slot, a `DropItem`, an item
/// booth's trade goods or a damage-drop table still points at -- walked
/// directly off the raw save, never through `remove_orphaned_dynamic_items`
/// or anything else `delete_unreferenced_data` calls.
fn referenced_dynamic_item_ids(session: &SaveSession) -> std::collections::HashSet<Uuid> {
    use psp_core::ue::games::palworld::{PalItemId, PalMapConcreteModelVariant};
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    fn item_dynamic_id(item: &PalItemId) -> Option<Uuid> {
        let id = props::guid_to_uuid(&item.dynamic_id.local_id_in_created_world);
        (!id.is_nil()).then_some(id)
    }
    fn concrete_variant(
        object: &StructValue,
    ) -> Option<&PalMapConcreteModelVariant<psp_core::ue::Arch>> {
        let StructValue::Struct(properties) = object else { return None };
        let concrete =
            properties.0.get(&PropertyKey::from("ConcreteModel")).and_then(props::struct_props)?;
        match concrete.0.get(&PropertyKey::from("RawData"))? {
            Property::Struct(StructValue::Game(PalStruct::MapConcreteModel(raw))) => {
                Some(&raw.model_data)
            }
            _ => None,
        }
    }

    let mut ids = std::collections::HashSet::new();

    if let Ok(entries) = session.item_container_map() {
        for entry in entries {
            let Some(value_props) = props::struct_props(&entry.value) else { continue };
            let Some(slot_values) =
                props::get(value_props, &["Slots"]).and_then(props::struct_values)
            else {
                continue;
            };
            for slot_value in slot_values {
                let StructValue::Struct(slot_props) = slot_value else { continue };
                if let Some(Property::Struct(StructValue::Game(PalStruct::ItemContainerSlots(raw)))) =
                    slot_props.0.get(&PropertyKey::from("RawData"))
                {
                    if let Some(id) = item_dynamic_id(&raw.item) {
                        ids.insert(id);
                    }
                }
            }
        }
    }

    if let Ok(Some(objects)) = psp_core::domain::world::map_object_values(&session.level) {
        for object in objects {
            let Some(variant) = concrete_variant(object) else { continue };
            match variant {
                PalMapConcreteModelVariant::DropItem(model) => {
                    if let Some(id) = item_dynamic_id(&model.item_id) {
                        ids.insert(id);
                    }
                }
                PalMapConcreteModelVariant::ItemBooth(model) => {
                    for trade in &model.trade_infos {
                        for item in [&trade.product, &trade.cost] {
                            if let Some(id) = item_dynamic_id(&item.item_id) {
                                ids.insert(id);
                            }
                        }
                    }
                }
                PalMapConcreteModelVariant::ItemDropOnDamag(model) => {
                    for item in &model.drop_item_infos {
                        if let Some(id) = item_dynamic_id(&item.item_id) {
                            ids.insert(id);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    ids
}

fn count_orphaned_dynamic_items(session: &SaveSession) -> usize {
    let referenced = referenced_dynamic_item_ids(session);
    dynamic_item_ids(session).iter().filter(|id| !referenced.contains(id)).count()
}

// --- per-command setup shared by test 11's round trip and test 12's dry-run gate ---

fn setup_for(command_id: &str, h: &mut Harness) -> serde_json::Value {
    match command_id {
        "delete_all_skins" => serde_json::json!({}),
        "delete_duplicated_players" => {
            seed_duplicate_player(&mut h.session);
            serde_json::json!({})
        }
        "delete_empty_guilds" => {
            manufacture_empty_guild(h);
            serde_json::json!({})
        }
        "delete_imported_pals" => {
            if count_pals_where(&h.session, |p| p.is_imported) == 0 {
                seed_imported_pal(&mut h.session);
            }
            serde_json::json!({})
        }
        "remove_invalid_pals_from_save" => {
            seed_pal_with_character_id(h, "PSP_NOT_A_REAL_PAL");
            serde_json::json!({})
        }
        "delete_inactive_bases" => {
            serde_json::json!({ "mode": "inactive", "days": 0, "level": 1 })
        }
        "delete_non_base_map_objects" => {
            orphan_one_base(&mut h.session);
            serde_json::json!({})
        }
        "delete_invalid_structure_map_objects" => {
            seed_map_object_id(&mut h.session, "PSP_NOT_A_REAL_STRUCTURE");
            serde_json::json!({})
        }
        "delete_unreferenced_data" => serde_json::json!({}),
        "delete_inactive_players" => {
            let max_elapsed = elapsed_days_since_last_online(&h.session);
            serde_json::json!({ "days": 1.max(max_elapsed - 20) })
        }
        "fix_all_negative_timestamps" => {
            let world_tick = world_now_tick(&h.session);
            let uid = h.session.player_summary_order[0];
            seed_last_online_real_time(&mut h.session, uid, world_tick + 1_000_000);
            serde_json::json!({})
        }
        "remove_invalid_items_from_save" => {
            seed_bogus_item(h);
            serde_json::json!({})
        }
        "remove_invalid_passives_from_save" => {
            seed_passive_skill(&mut h.session, BOGUS_PASSIVE_ID);
            serde_json::json!({})
        }
        other => panic!("no setup defined for {other}"),
    }
}

// --- 1 ---

#[test]
fn the_bundled_manifest_parses_and_declares_every_command() {
    let plugin = &BUNDLED[0];
    assert_eq!(plugin.id, "pst.cleanup");
    let manifest =
        Manifest::parse(plugin.manifest).expect("the bundled manifest must parse");
    assert_eq!(manifest.commands.len(), 13);
    let mut ids: Vec<&str> = manifest.commands.iter().map(|c| c.id.as_str()).collect();
    ids.sort_unstable();
    let mut expected: Vec<&str> = ALL_COMMANDS.to_vec();
    expected.sort_unstable();
    assert_eq!(ids, expected);
}

// --- 1b ---

#[test]
fn delete_all_skins_removes_every_skin_field_and_the_save_still_parses() {
    let mut h = Harness::new();

    let before = count_raw_keys(&h.session, &["SkinName", "SkinAppliedCharacterId"]);
    assert!(before > 0, "the fixture must carry skin fields, or this test is vacuous");

    let dry = h.run("delete_all_skins", serde_json::json!({}), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    assert_eq!(
        count_raw_keys(&h.session, &["SkinName", "SkinAppliedCharacterId"]),
        before,
        "a dry run must not change the save"
    );

    let wet = h.run("delete_all_skins", serde_json::json!({}), false);
    assert_eq!(wet.status, RunStatus::Ok, "{:?}", wet.status);
    assert_eq!(wet.counts.get("skins").copied(), Some(before as i64));
    assert_eq!(count_raw_keys(&h.session, &["SkinName", "SkinAppliedCharacterId"]), 0);
    assert_round_trips(&h.session);
}

#[test]
fn delete_all_skins_clears_the_stored_skin_inventory_of_every_player() {
    let mut h = Harness::new();

    let first = h.run("delete_all_skins", serde_json::json!({}), false);
    assert_eq!(first.status, RunStatus::Ok, "{:?}", first.status);
    let cleared = first.counts.get("player_skin_inventories").copied().unwrap_or(0);
    assert!(
        cleared > 0,
        "the fixture's players carry a stored skin inventory; clearing none means the          player-side branch did nothing"
    );

    let second = h.run("delete_all_skins", serde_json::json!({}), false);
    assert_eq!(second.status, RunStatus::Ok, "{:?}", second.status);
    assert_eq!(
        second.counts.get("player_skin_inventories").copied(),
        Some(0),
        "the first run must have removed them; a branch that reports without writing would          clear the same records again"
    );
}

// --- 2 ---

#[test]
fn delete_empty_guilds_removes_exactly_the_guilds_with_no_players() {
    let mut h = Harness::new();
    let target_guild = manufacture_empty_guild(&mut h);
    let untouched_before: BTreeMap<Uuid, i64> = h
        .session
        .guild_summaries
        .iter()
        .filter(|(id, _)| **id != target_guild)
        .map(|(id, g)| (*id, g.player_count))
        .collect();

    let outcome = h.run("delete_empty_guilds", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("guilds").copied(), Some(1));
    assert_eq!(
        outcome.counts.get("unresolved").copied(),
        Some(0),
        "the manufactured guild resolves cleanly; a nonzero here would mean a real host-level \
         skip went unreported"
    );
    assert!(
        !h.session.guild_summaries.contains_key(&target_guild),
        "the manufactured empty guild must be removed"
    );

    for (id, before) in untouched_before {
        let after = h.session.guild_summaries.get(&id).map(|g| g.player_count);
        assert_eq!(after, Some(before), "guild {id} must be spared");
    }
}

// --- 3 ---

#[test]
fn delete_empty_guilds_on_a_save_with_none_reports_zero() {
    let mut h = Harness::new();
    assert!(
        h.session.guild_summaries.values().all(|g| g.player_count != 0),
        "the unmodified fixture must have no zero-player guild (see the task report)"
    );
    let guild_count_before = h.session.guild_summaries.len();

    let outcome = h.run("delete_empty_guilds", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("guilds").copied(), Some(0));
    assert_eq!(outcome.counts.get("unresolved").copied(), Some(0));
    assert_eq!(h.session.guild_summaries.len(), guild_count_before);
}

// --- 4 ---

#[test]
fn delete_inactive_players_respects_its_days_parameter() {
    let max_elapsed = elapsed_days_since_last_online(&Harness::new().session);
    let short_days = 1.max(max_elapsed - 20);
    let long_days = max_elapsed + 20;

    let mut short_h = Harness::new();
    let short_outcome =
        short_h.run("delete_inactive_players", serde_json::json!({ "days": short_days }), false);
    assert_eq!(short_outcome.status, RunStatus::Ok, "{:?}", short_outcome.status);
    let short_removed = short_outcome.counts.get("players").copied().unwrap_or(0);

    let mut long_h = Harness::new();
    let long_outcome =
        long_h.run("delete_inactive_players", serde_json::json!({ "days": long_days }), false);
    assert_eq!(long_outcome.status, RunStatus::Ok, "{:?}", long_outcome.status);
    let long_removed = long_outcome.counts.get("players").copied().unwrap_or(0);

    assert!(
        short_removed > long_removed,
        "a shorter inactivity window must remove more players than a longer one \
         (short={short_days}d -> {short_removed}, long={long_days}d -> {long_removed})"
    );
    assert_eq!(long_removed, 0, "a threshold older than every login must remove nobody");
}

// --- 5 ---

#[test]
fn delete_inactive_players_never_removes_a_guild_admin() {
    let mut h = Harness::new();
    let loaded_admins: Vec<Uuid> = h
        .session
        .guild_summaries
        .values()
        .filter_map(|g| g.admin_player_uid)
        .filter(|uid| h.session.player_summaries.contains_key(uid))
        .collect();
    assert!(!loaded_admins.is_empty(), "the fixture must have at least one loaded guild admin");

    let aggressive_days = 1.max(elapsed_days_since_last_online(&h.session) - 20);
    let outcome =
        h.run("delete_inactive_players", serde_json::json!({ "days": aggressive_days }), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert!(
        outcome.counts.get("admins_skipped").copied().unwrap_or(0) > 0,
        "this aggressive a threshold must have encountered at least one guild admin to skip"
    );
    assert_eq!(
        outcome.counts.get("unresolved").copied(),
        Some(0),
        "every fixture player resolves cleanly; a nonzero here would mean a real host-level \
         skip went unreported"
    );

    for admin in loaded_admins {
        assert!(
            h.session.player_summaries.contains_key(&admin),
            "guild admin {admin} must never be removed"
        );
    }
}

// --- 6 ---

#[test]
fn delete_inactive_players_with_a_huge_threshold_removes_nobody() {
    let mut h = Harness::new();
    let outcome = h.run("delete_inactive_players", serde_json::json!({ "days": 3650 }), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("players").copied(), Some(0));
    assert_eq!(outcome.counts.get("unresolved").copied(), Some(0));
}

// --- 6a ---

#[test]
fn the_fixture_has_players_with_readable_last_online_timestamps() {
    let h = Harness::new();
    let with_timestamp =
        h.session.player_summaries.values().filter(|p| p.last_online_time.is_some()).count();
    assert!(
        with_timestamp > 0,
        "PlayerSummary.last_online_time is None for every fixture player; \
         tests 4 and 6 are unexercised, not merely passing vacuously -- see the task report"
    );
}

// --- 6b ---

#[test]
fn delete_duplicated_players_keeps_the_most_recently_online_copy() {
    let mut h = Harness::new();
    let (uid, kept_instance, stale_instance) = seed_duplicate_player(&mut h.session);
    let before = count_character_entries(&h.session);

    let outcome = h.run("delete_duplicated_players", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("players").copied(), Some(1));

    assert_eq!(count_character_entries(&h.session), before - 1);
    assert!(character_entry_exists(&h.session, kept_instance), "the recent copy survives");
    assert!(!character_entry_exists(&h.session, stale_instance), "the stale copy is gone");
    assert_eq!(
        count_character_entries_for_uid(&h.session, uid),
        1,
        "exactly one entry may remain for the duplicated uid"
    );
    assert_round_trips(&h.session);
}

#[test]
fn delete_duplicated_players_does_nothing_to_a_save_without_duplicates() {
    let mut h = Harness::new();
    let before = count_character_entries(&h.session);
    let outcome = h.run("delete_duplicated_players", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("players").copied(), Some(0));
    assert_eq!(count_character_entries(&h.session), before);
}

// --- 7 ---

#[test]
fn fix_all_negative_timestamps_clamps_a_seeded_future_timestamp() {
    let mut h = Harness::new();
    let world_tick = world_now_tick(&h.session);
    let target_uid = h.session.player_summary_order[0];
    seed_last_online_real_time(&mut h.session, target_uid, world_tick + 1_000_000);

    let outcome = h.run("fix_all_negative_timestamps", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("timestamps").copied(), Some(1));

    let after = read_last_online_real_time(&h.session, target_uid)
        .expect("the seeded value must still be present");
    assert_eq!(after, world_tick);
}

// --- 8 ---

#[test]
fn fix_all_negative_timestamps_leaves_a_valid_timestamp_alone() {
    let mut h = Harness::new();
    let world_tick = world_now_tick(&h.session);
    let target_uid = h.session.player_summary_order[1];
    let valid_value = world_tick - 1_000_000;
    seed_last_online_real_time(&mut h.session, target_uid, valid_value);

    let outcome = h.run("fix_all_negative_timestamps", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("timestamps").copied(), Some(0));

    let after = read_last_online_real_time(&h.session, target_uid)
        .expect("the seeded value must still be present");
    assert_eq!(after, valid_value, "a timestamp that is not in the future must be left alone");
}

// --- 10 ---

#[test]
fn remove_invalid_items_clears_a_seeded_bogus_item_and_keeps_valid_ones() {
    let mut h = Harness::new();
    let (container_id, slot_index) = seed_bogus_item(&mut h);

    let outcome = h.run("remove_invalid_items_from_save", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let cleared = outcome.counts.get("slots").copied().unwrap_or(0);
    let checked = outcome.counts.get("checked").copied().unwrap_or(0);
    assert!(
        cleared >= 1,
        "the seeded bogus stack must be cleared; summary was {:?}, counts {:?}",
        outcome.summary,
        outcome.counts
    );
    assert!(checked > cleared, "other valid item stacks must still be present to check");

    let after = containers::read_item_container(
        &h.session.level,
        &mut h.session.caches,
        &h.game_data,
        container_id,
        "",
        None,
    )
    .expect("the container still resolves after the run");
    let still_bogus = after
        .slots
        .iter()
        .find(|slot| slot.slot_index == slot_index)
        .and_then(|slot| slot.static_id.as_deref())
        == Some(BOGUS_ITEM_ID);
    assert!(!still_bogus, "the bogus item must be gone after the command runs");
}

// --- 10a ---

#[test]
fn delete_imported_pals_removes_only_the_imported_ones() {
    let mut h = Harness::new();
    if count_pals_where(&h.session, |p| p.is_imported) == 0 {
        seed_imported_pal(&mut h.session);
    }
    let imported = count_pals_where(&h.session, |p| p.is_imported);
    let total = count_pals_where(&h.session, |_| true);
    assert!(imported > 0, "seed an imported pal rather than asserting zero");
    assert!(imported < total, "not every pal may be imported, or the test proves nothing");

    let outcome = h.run("delete_imported_pals", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("pals").copied(), Some(imported as i64));
    assert_eq!(count_pals_where(&h.session, |p| p.is_imported), 0);
    assert_eq!(count_pals_where(&h.session, |_| true), total - imported);
    assert_round_trips(&h.session);
}

// --- 10b ---

#[test]
fn remove_invalid_pals_spares_bosses_and_predators() {
    let mut h = Harness::new();
    let bosses = count_pals_where(&h.session, |p| p.is_boss);
    assert!(bosses > 0, "seed a boss rather than asserting zero");

    let outcome = h.run("remove_invalid_pals_from_save", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(
        count_pals_where(&h.session, |p| p.is_boss),
        bosses,
        "a boss id is not in the pal catalog verbatim; deleting one is the failure this guards"
    );
    assert_round_trips(&h.session);
}

// --- 10c ---

#[test]
fn remove_invalid_pals_deletes_a_seeded_unknown_species() {
    let mut h = Harness::new();
    let before = count_pals_where(&h.session, |_| true);
    seed_pal_with_character_id(&mut h, "PSP_NOT_A_REAL_PAL");

    let outcome = h.run("remove_invalid_pals_from_save", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("pals").copied(), Some(1));
    assert_eq!(count_pals_where(&h.session, |_| true), before);
    assert_round_trips(&h.session);
}

// --- 10d ---

#[test]
fn delete_imported_pals_empties_a_dps_slot_without_shrinking_the_array() {
    let mut h = Harness::new();
    let uid = dps_player_uid();
    let slot_index = seed_dps_imported_pal(&mut h, uid);

    let before_dps = dps_bytes_for(&h, uid);
    let before_count = dps_slot_count(&before_dps);
    assert_eq!(
        dps_slot_character_id(&before_dps, slot_index).as_deref(),
        Some("Lamball"),
        "the seeded dps pal must be readable before the command runs"
    );

    let outcome = h.run("delete_imported_pals", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("dimensional_storage_pals").copied(), Some(1));

    let after_dps = dps_bytes_for(&h, uid);
    assert_eq!(
        dps_slot_count(&after_dps), before_count,
        "emptying an imported dps pal must not change the storage array's length"
    );
    assert_eq!(
        dps_slot_character_id(&after_dps, slot_index).as_deref(),
        Some("None"),
        "the emptied slot must read back as an unused slot, not have been removed"
    );
}

// --- 10e ---

#[test]
fn remove_invalid_pals_empties_a_dps_slot_without_shrinking_the_array() {
    let mut h = Harness::new();
    let uid = dps_player_uid();
    let slot_index = seed_dps_invalid_pal(&mut h, uid);

    let before_dps = dps_bytes_for(&h, uid);
    let before_count = dps_slot_count(&before_dps);
    assert_eq!(
        dps_slot_character_id(&before_dps, slot_index).as_deref(),
        Some("PSP_NOT_A_REAL_PAL"),
        "the seeded dps pal must be readable before the command runs"
    );

    let outcome = h.run("remove_invalid_pals_from_save", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("dimensional_storage_pals").copied(), Some(1));

    let after_dps = dps_bytes_for(&h, uid);
    assert_eq!(
        dps_slot_count(&after_dps), before_count,
        "emptying an invalid dps pal must not change the storage array's length"
    );
    assert_eq!(
        dps_slot_character_id(&after_dps, slot_index).as_deref(),
        Some("None"),
        "the emptied slot must read back as an unused slot, not have been removed"
    );
}

// --- 10f ---

#[test]
fn remove_invalid_passives_strips_a_seeded_unknown_and_keeps_the_rest() {
    let mut h = Harness::new();
    let target = seed_passive_skill(&mut h.session, "PSP_NOT_A_REAL_PASSIVE");
    let before = count_passive_entries(&h.session);
    assert!(before > 1, "the fixture must carry real passives too");
    assert!(
        passives_of(&h.session, target).iter().any(|p| p == "PSP_NOT_A_REAL_PASSIVE"),
        "the seed must be readable before the command runs, or this test proves nothing"
    );

    let dry = h.run("remove_invalid_passives_from_save", serde_json::json!({}), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    assert_eq!(count_passive_entries(&h.session), before, "a dry run changes nothing");

    let wet = h.run("remove_invalid_passives_from_save", serde_json::json!({}), false);
    assert_eq!(wet.status, RunStatus::Ok, "{:?}", wet.status);
    assert_eq!(wet.counts.get("passives").copied(), Some(1));
    assert_eq!(count_passive_entries(&h.session), before - 1);
    assert!(
        !passives_of(&h.session, target).iter().any(|p| p == "PSP_NOT_A_REAL_PASSIVE"),
        "the seeded skill must be gone from the pal it was seeded on"
    );
    assert_round_trips(&h.session);
}

// --- 10g ---

#[test]
fn remove_invalid_passives_strips_a_seeded_unknown_from_dimensional_storage_and_keeps_the_rest() {
    let mut h = Harness::new();
    let uid = dps_player_uid();
    let (slot_index, valid_skill) = seed_dps_passive_skill(&mut h, uid);

    let before_dps = dps_bytes_for(&h, uid);
    let before_skills = dps_slot_passive_skills(&before_dps, slot_index);
    assert!(
        before_skills.iter().any(|s| s == BOGUS_PASSIVE_ID) && before_skills.contains(&valid_skill),
        "both skills must be readable before the command runs, got {before_skills:?}"
    );

    let outcome = h.run("remove_invalid_passives_from_save", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("dimensional_storage_passives").copied(), Some(1));

    let after_dps = dps_bytes_for(&h, uid);
    let after_skills = dps_slot_passive_skills(&after_dps, slot_index);
    assert!(
        !after_skills.iter().any(|s| s == BOGUS_PASSIVE_ID),
        "the seeded invalid passive must be gone, got {after_skills:?}"
    );
    assert!(
        after_skills.contains(&valid_skill),
        "the valid passive on the same pal must survive, got {after_skills:?}"
    );
}

// --- 10h ---

#[test]
fn delete_inactive_bases_removes_only_bases_whose_members_all_fail_the_filter() {
    let mut h = Harness::new();
    let before = count_bases(&h.session);
    assert!(before > 0, "the fixture must have bases");

    let none = h.run(
        "delete_inactive_bases",
        serde_json::json!({ "mode": "inactive", "days": 36500, "level": 1 }),
        false,
    );
    assert_eq!(none.status, RunStatus::Ok, "{:?}", none.status);
    assert_eq!(
        none.counts.get("bases").copied(),
        Some(0),
        "at a 100-year threshold nobody is inactive"
    );
    assert_eq!(count_bases(&h.session), before);

    let all = h.run(
        "delete_inactive_bases",
        serde_json::json!({ "mode": "inactive", "days": 0, "level": 1 }),
        false,
    );
    assert_eq!(all.status, RunStatus::Ok, "{:?}", all.status);
    let removed = all.counts.get("bases").copied().unwrap_or(0);
    assert!(removed > 0, "at a zero-day threshold every known member is inactive");
    assert_eq!(count_bases(&h.session), before - removed as usize);
    assert_round_trips(&h.session);
}

#[test]
fn a_base_whose_guild_has_no_visible_members_is_skipped_not_deleted() {
    let mut h = Harness::new();
    let orphan_base = seed_base_with_no_visible_members(&mut h);
    let outcome = h.run(
        "delete_inactive_bases",
        serde_json::json!({ "mode": "inactive", "days": 0, "level": 1 }),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert!(
        base_exists(&h.session, orphan_base),
        "no visible members means unknown, not inactive"
    );
    assert!(outcome.counts.get("skipped_unknown").copied().unwrap_or(0) > 0);
}

// --- 10i ---

#[test]
fn delete_non_base_map_objects_removes_exactly_what_the_deleted_base_owned() {
    let mut h = Harness::new();
    let (orphaned_base, expected) = orphan_one_base(&mut h.session);
    assert!(expected > 0, "the chosen base must own map objects, or the test is vacuous");
    let before = count_map_objects(&h.session);

    let outcome = h.run("delete_non_base_map_objects", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("map_objects").copied(), Some(expected as i64));
    assert_eq!(count_map_objects(&h.session), before - expected);
    assert!(
        !any_map_object_belongs_to(&h.session, orphaned_base),
        "nothing may still point at the removed base"
    );
    assert_round_trips(&h.session);
}

#[test]
fn delete_non_base_map_objects_spares_everything_still_attached() {
    let mut h = Harness::new();
    let attached_before = count_map_objects_where(&h.session, |v| v.base_id.is_some());
    assert_eq!(attached_before, 2144, "the fixture's attached map objects");

    let outcome = h.run("delete_non_base_map_objects", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("map_objects").copied(), Some(0));
    assert_eq!(count_map_objects_where(&h.session, |v| v.base_id.is_some()), attached_before);
}

#[test]
fn delete_non_base_map_objects_never_touches_world_content_that_has_no_base() {
    let mut h = Harness::new();
    let unattached_before = count_map_objects_where(&h.session, |v| v.base_id.is_none());
    assert_eq!(unattached_before, 3308, "the fixture's world props, chests and resource nodes");

    orphan_one_base(&mut h.session);
    let outcome = h.run("delete_non_base_map_objects", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(
        count_map_objects_where(&h.session, |v| v.base_id.is_none()),
        unattached_before,
        "an object with no base at all is world content, not orphaned construction"
    );
}

#[test]
fn delete_invalid_structure_map_objects_keeps_treasure_boxes_and_resource_nodes() {
    let mut h = Harness::new();
    let treasure_before = count_map_objects_where(&h.session, |v| {
        let id = v.map_object_id.to_ascii_lowercase();
        id.starts_with("treasurebox") || id.starts_with("damagable") || id == "commondropitem3d"
    });
    assert!(treasure_before > 0, "the fixture must carry non-catalog world props");

    let outcome = h.run("delete_invalid_structure_map_objects", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(
        count_map_objects_where(&h.session, |v| {
            let id = v.map_object_id.to_ascii_lowercase();
            id.starts_with("treasurebox") || id.starts_with("damagable") || id == "commondropitem3d"
        }),
        treasure_before,
        "these are legitimate world props, not invalid structures"
    );
    assert_eq!(outcome.counts.get("map_objects").copied(), Some(0));
}

#[test]
fn delete_invalid_structure_map_objects_removes_a_seeded_unknown_structure() {
    let mut h = Harness::new();
    let before = count_map_objects(&h.session);
    seed_map_object_id(&mut h.session, "PSP_NOT_A_REAL_STRUCTURE");

    let outcome = h.run("delete_invalid_structure_map_objects", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("map_objects").copied(), Some(1));
    assert_eq!(count_map_objects(&h.session), before - 1);
    assert_round_trips(&h.session);
}

#[test]
fn a_structure_id_matches_the_catalog_case_insensitively() {
    let mut h = Harness::new();
    seed_map_object_id(&mut h.session, "sToNe_FoUnDaTiOn");
    let outcome = h.run("delete_invalid_structure_map_objects", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(
        outcome.counts.get("map_objects").copied(),
        Some(0),
        "Stone_Foundation is a real structure whatever its casing"
    );
}

#[test]
fn removing_map_objects_takes_their_work_entries_with_them() {
    let mut h = Harness::new();
    let (_, _) = orphan_one_base(&mut h.session);
    let works_before = count_work_entries(&h.session);

    let outcome = h.run("delete_non_base_map_objects", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let works_removed = outcome.counts.get("works").copied().unwrap_or(0);
    assert!(works_removed > 0, "a base's structures carry work entries; seed one if this fails");
    assert_eq!(count_work_entries(&h.session), works_before - works_removed as usize);
    assert_eq!(count_orphaned_works(&h.session), 0, "no work may dangle after the sweep");
    assert_round_trips(&h.session);
}

// --- 10j ---

#[test]
fn delete_unreferenced_data_removes_orphans_and_leaves_referenced_data_alone() {
    let mut h = Harness::new();
    let items_before = count_dynamic_items(&h.session);
    let orphans = count_orphaned_dynamic_items(&h.session);
    assert!(orphans > 0, "the fixture must carry orphaned dynamic items");
    assert!(orphans < items_before, "not all of them may be orphans");
    let referenced = referenced_dynamic_item_ids(&h.session);
    for id in &referenced {
        assert!(
            dynamic_item_exists(&h.session, *id),
            "a referenced dynamic item must be readable before the command runs"
        );
    }

    let dry = h.run("delete_unreferenced_data", serde_json::json!({}), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    assert_eq!(count_dynamic_items(&h.session), items_before, "a dry run changes nothing");

    let wet = h.run("delete_unreferenced_data", serde_json::json!({}), false);
    assert_eq!(wet.status, RunStatus::Ok, "{:?}", wet.status);
    assert_eq!(wet.counts.get("dynamic_items").copied(), Some(orphans as i64));
    assert_eq!(count_dynamic_items(&h.session), items_before - orphans);

    for id in referenced {
        assert!(
            dynamic_item_exists(&h.session, id),
            "a referenced dynamic item was removed; the save now dereferences a missing entry"
        );
    }
    assert_eq!(count_orphaned_dynamic_items(&h.session), 0);
    assert_round_trips(&h.session);
}

#[test]
fn delete_unreferenced_data_removes_exactly_the_pals_the_independent_walk_finds() {
    let mut h = Harness::new();
    let expected = count_ownerless_pals(&h.session);
    assert!(expected > 0, "the fixture must carry ownerless pals");
    let before = h.session.character_map().expect("character map").len();

    let outcome = h.run("delete_unreferenced_data", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("pals").copied(), Some(expected as i64));
    assert_eq!(count_ownerless_pals(&h.session), 0);
    assert_eq!(h.session.character_map().expect("character map").len(), before - expected);
    assert_round_trips(&h.session);
}

#[test]
fn delete_unreferenced_data_clears_stale_structure_references_naturally_present_in_the_fixture() {
    let mut h = Harness::new();
    let expected = count_stale_map_object_builders(&h.session);
    assert!(expected > 0, "the fixture must carry stale structure references");

    let outcome = h.run("delete_unreferenced_data", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("references").copied(), Some(expected as i64));
    assert_eq!(count_stale_map_object_builders(&h.session), 0);
    assert_round_trips(&h.session);
}

#[test]
fn delete_unreferenced_data_clears_a_map_object_reference_to_a_removed_player() {
    let mut h = Harness::new();
    let (object_id, ghost_uid) = seed_map_object_built_by_missing_player(&mut h.session);

    let outcome = h.run("delete_unreferenced_data", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert!(outcome.counts.get("references").copied().unwrap_or(0) > 0);
    assert_ne!(
        map_object_builder(&h.session, object_id),
        Some(ghost_uid),
        "a reference to a player who is not in the save must be cleared"
    );
    assert_round_trips(&h.session);
}

#[test]
fn delete_unreferenced_data_is_idempotent() {
    let mut h = Harness::new();
    let first = h.run("delete_unreferenced_data", serde_json::json!({}), false);
    assert_eq!(first.status, RunStatus::Ok, "{:?}", first.status);

    let second = h.run("delete_unreferenced_data", serde_json::json!({}), false);
    assert_eq!(second.status, RunStatus::Ok, "{:?}", second.status);
    for key in ["dynamic_items", "works", "pals", "references"] {
        assert_eq!(
            second.counts.get(key).copied(),
            Some(0),
            "a second sweep must find nothing left: {key}"
        );
    }
}

// --- 11 ---

#[test]
fn every_command_round_trips_the_save_through_write_and_reparse() {
    for command_id in ALL_COMMANDS {
        let mut h = Harness::new();
        let args = setup_for(command_id, &mut h);

        let outcome = h.run(command_id, args, false);
        assert_eq!(outcome.status, RunStatus::Ok, "{command_id}: {:?}", outcome.status);

        let bytes = h
            .session
            .level_sav_bytes()
            .unwrap_or_else(|e| panic!("{command_id}: level_sav_bytes failed: {e}"));
        reparse(&bytes).unwrap_or_else(|e| panic!("{command_id}: the written save did not reparse: {e}"));
    }
}

// --- 12 ---

#[test]
fn every_destructive_command_has_matching_dry_run_and_real_counts() {
    for command_id in ALL_COMMANDS {
        let mut h = Harness::new();
        let args = setup_for(command_id, &mut h);

        let before = h.session.level_sav_bytes().expect("level_sav_bytes before the dry run");
        let dry = h.run(command_id, args.clone(), true);
        assert_eq!(dry.status, RunStatus::Ok, "{command_id} dry: {:?}", dry.status);
        let after_dry = h.session.level_sav_bytes().expect("level_sav_bytes after the dry run");
        assert_eq!(before, after_dry, "{command_id}: a dry run must not change level_sav_bytes()");

        let real = h.run(command_id, args, false);
        assert_eq!(real.status, RunStatus::Ok, "{command_id} real: {:?}", real.status);
        assert!(!real.counts.is_empty(), "{command_id}: expected at least one count key");

        // `RunOutcome::counts` carries extra internal preview keys under a
        // dry run (see its own doc comment) that a real run never populates,
        // so the two maps are not required to be equal wholesale -- every
        // key the SCRIPT itself reports (`summary`'s paired `counts` table,
        // which is what the manifest promises the user) must agree between
        // the two, which is what this checks.
        for (key, value) in &real.counts {
            assert_eq!(
                dry.counts.get(key),
                Some(value),
                "{command_id}: dry/real count mismatch for {key:?} (dry={:?} real={:?})",
                dry.counts,
                real.counts
            );
        }
    }
}

// --- 13 ---

/// Two bundled plugins must both seed and both resolve. A registry that
/// silently served only the first would still pass every single-plugin test
/// in this file.
#[test]
fn both_bundled_plugins_are_registered_with_distinct_ids() {
    let ids: Vec<&str> = psp_app::plugin_registry::BUNDLED.iter().map(|p| p.id).collect();
    assert!(ids.contains(&"pst.cleanup"), "got {ids:?}");
    assert!(ids.contains(&"pst.reset"), "got {ids:?}");
    assert_eq!(
        ids.len(),
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        "bundled plugin ids must be unique: {ids:?}"
    );
}
