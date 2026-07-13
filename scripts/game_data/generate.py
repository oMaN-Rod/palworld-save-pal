"""Regenerate PSP static game data from an FModel/UnrealExporter PAK dump.

Usage:
    python scripts/game_data/generate.py --dump-dir <datamine>/extracted/Pal/Content \
        [--only pals,active_skills,passive_skills,l10n] [--check]

The dump directory must contain Pal/DataTable/** and L10N/** JSON exports
(see scripts/game_data/README.md for how to produce one).

Merge policy: existing entries are refreshed from the dump (the game rebalances
stats between versions), hand-curated fields with no dump source (`disabled`,
existing `icon` overrides) are preserved, existing key order is kept, and new
keys are appended alphabetically. --check reports the diff without writing.
"""

import argparse
import json
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from dt_reader import (  # noqa: E402
    _PLACEHOLDER,
    load_psp_json,
    localized_string,
    read_rows,
    save_psp_json,
    strip_enum,
)


def _clean(value: str | None, key: str | None = None) -> str | None:
    """None out per-language placeholder strings ("ko_Text") that leaked into
    previously committed l10n files. When the entry key is given, a committed
    value that is just the raw code name ("AnimalSkin2") is dropped too —
    natural display names never look like code identifiers."""
    if isinstance(value, str) and (
            _PLACEHOLDER.match(value.strip()) or value.strip() == "-"):
        return None
    # Unresolved RichText remnants from older pipeline runs ("<itemName
    # id=|X|/>") mean the committed value was never valid display text.
    if isinstance(value, str) and "<" in value and ("id=|" in value or "/>" in value):
        return None
    # Known-bad strings committed by older generator runs (upstream included):
    # raw code leaks for rows the game never authored in any language.
    if isinstance(value, str) and value.strip().startswith(_BAD_COMMITTED_PREFIXES):
        return None
    if (key is not None and isinstance(value, str) and value == key.split("::")[-1]
            and re.search(r"[_0-9]|[a-z][A-Z]", value)):
        return None
    return value

REPO_ROOT = Path(__file__).resolve().parents[2]
DATA_DIR = REPO_ROOT / "data" / "json"
L10N_DIR = DATA_DIR / "l10n"

# Genus categories PSP's UI understands; everything else renders as Unknown.
KNOWN_GENUS = {"Humanoid", "Bird", "Fish", "Dragon"}

# Passive-skill effect types/targets the UI understands (ui/src/lib/types/game.ts,
# EffectType / TargetType enums). Values the UI doesn't know are recorded as
# "None" — descriptions still come from l10n, so nothing is lost visually.
KNOWN_EFFECT_TYPES = {
    "no", "MaxHP", "MeleeAttack", "ShotAttack", "Defense", "Support",
    "CraftSpeed", "MoveSpeed", "Homing", "Explosive", "BulletSpeed",
    "BulletAccuracy", "Recoil", "ElementFire", "ElementWater", "ElementLeaf",
    "ElementElectricity", "ElementIce", "ElementEarth", "ElementDark",
    "ElementDragon", "ElementResist_Normal", "ElementResist_Fire",
    "ElementResist_Water", "ElementResist_Leaf", "ElementResist_Electricity",
    "ElementResist_Ice", "ElementResist_Earth", "ElementResist_Dark",
    "ElementResist_Dragon", "ElementBoost_Normal", "ElementBoost_Fire",
    "ElementBoost_Water", "ElementBoost_Leaf", "ElementBoost_Electricity",
    "ElementBoost_Ice", "ElementBoost_Earth", "ElementBoost_Dark",
    "ElementBoost_Dragon", "ElementAddItemDrop_Normal",
    "ElementAddItemDrop_Fire", "ElementAddItemDrop_Water",
    "ElementAddItemDrop_Leaf", "ElementAddItemDrop_Electricity",
    "ElementAddItemDrop_Ice", "ElementAddItemDrop_Earth",
    "ElementAddItemDrop_Dark", "ElementAddItemDrop_Dragon",
    "MoveSpeed_Ground", "MoveSpeed_Wood", "MoveSpeed_Grass",
    "MoveSpeed_Stone", "MoveSpeed_Water", "MoveSpeed_Snow", "MoveSpeed_Lava",
    "CollectItem", "Mute", "Logging", "Mining", "GainItemDrop",
    "CollectItemDrop", "LifeSteal", "TemperatureResist_Heat",
    "TemperatureResist_Cold", "TemperatureInvalid_Heat",
    "TemperatureInvalid_Cold", "MaxInventoryWeight", "FullStomatch_Decrease",
    "Sanity_Decrease", "BodyPartsWeakDamage", "NonKilling",
    "ItemWeightReduction", "PalExp_Increase", "PalSP_Increase",
    "ShopBuyPrice_Money_Increase", "ShopSellPrice_Money_Increase",
    "BreedSpeed", "Nocturnal", "JumpPower_Increase", "JumpCount_Increase",
    "PalEggHatchingSpeed", "FarmCropGrowupSpeed", "SyncroPassiveWhenCapture",
    "ActiveSkillCoolTime_Decrease", "EPalPassiveSkillEffectType_MAX", "None",
}
KNOWN_EFFECT_TARGETS = {
    "ToSelf", "ToTrainer", "ToSelfAndTrainer", "ToBaseCampPal",
    "ToBuildObject", "EPalPassiveSkillEffectTargetType_MAX", "None",
}

WORK_SUITABILITIES = [
    "EmitFlame", "Watering", "Seeding", "GenerateElectricity", "Handcraft",
    "Collection", "Deforest", "Mining", "OilExtraction", "ProductMedicine",
    "Cool", "Transport", "MonsterFarm",
]

# PSP language dir -> pak L10N dir (identical unless listed here)
LANG_SOURCE_OVERRIDES = {"id-id": "id"}


def norm_key_map(d: dict) -> dict:
    """lowercase key -> actual key"""
    return {k.lower(): k for k in d}


# ---------------------------------------------------------------- pals

def build_pals(dump: Path, existing: dict) -> dict:
    mon = read_rows(dump, "Pal/DataTable/Character/DT_PalMonsterParameter.json")
    hum = read_rows(dump, "Pal/DataTable/Character/DT_PalHumanParameter.json")
    waza_levels = read_rows(dump, "Pal/DataTable/Waza/DT_WazaMasterLevel_Common.json")
    char_icons = read_rows(dump, "Pal/DataTable/Character/DT_PalCharacterIconDataTable.json")

    def icon_for(key: str, old: dict | None) -> str:
        row = char_icons.get(key)
        if row:  # the game's own icon mapping (covers NPC/variant sharing)
            asset = row["Icon"]["AssetPathName"]
            return asset.rsplit("/", 1)[-1].split(".")[0].lower()
        return (old or {}).get("icon") or f"t_{key.lower()}_icon_normal"

    skill_sets: dict[str, dict] = defaultdict(dict)
    for row in waza_levels.values():
        # column casing differs between exports ("PalId" vs "PalID")
        pal_id = row.get("PalId") or row.get("PalID")
        skill_sets[pal_id.lower()][strip_enum(row["WazaID"])] = row["Level"]

    def make_entry(key: str, row: dict, old: dict | None) -> dict:
        genus = strip_enum(row["GenusCategory"])
        if genus not in KNOWN_GENUS:
            genus = "Unknown"
        elements = [
            strip_enum(row[f])
            for f in ("ElementType1", "ElementType2")
            if strip_enum(row[f]) != "None"
        ]
        passives = [
            strip_enum(row[f])
            for f in ("PassiveSkill1", "PassiveSkill2", "PassiveSkill3", "PassiveSkill4")
            if strip_enum(row[f]) != "None"
        ]
        skill_set = skill_sets.get(key.lower(), {})
        skill_set = dict(sorted(skill_set.items(), key=lambda kv: kv[1]))
        return {
            "is_pal": row["IsPal"],
            "tribe": strip_enum(row["Tribe"]),
            "pal_deck_index": row["ZukanIndex"],
            "size": strip_enum(row["Size"]),
            "rarity": row["Rarity"],
            "element_types": elements,
            "genus_category": genus,
            "weapon": strip_enum(row["Weapon"]),
            "weapon_equip": row["WeaponEquip"],
            "scaling": {
                "hp": row["Hp"],
                "attack": row["ShotAttack"],
                "defense": row["Defense"],
            },
            "friendship_hp": row["Friendship_HP"],
            "friendship_shotattack": row["Friendship_ShotAttack"],
            "friendship_defense": row["Friendship_Defense"],
            "friendship_craftspeed": row["Friendship_CraftSpeed"],
            "enemy_max_hp_rate": row["EnemyMaxHPRate"],
            "enemy_receive_damage_rate": row["EnemyReceiveDamageRate"],
            "enemy_inflict_damage_rate": row["EnemyInflictDamageRate"],
            "capture_rate_correct": row["CaptureRateCorrect"],
            "exp_ratio": row["ExpRatio"],
            "price": row["Price"],
            "slow_walk_speed": row["SlowWalkSpeed"],
            "walk_speed": row["WalkSpeed"],
            "run_speed": row["RunSpeed"],
            "ride_sprint_speed": row["RideSprintSpeed"],
            "transport_speed": row["TransportSpeed"],
            "is_boss": row["IsBoss"],
            "is_tower_boss": row["IsTowerBoss"],
            "is_raid_boss": row["IsRaidBoss"],
            "nocturnal": row["Nocturnal"],
            "predator": row["Predator"],
            "edible": row["Edible"],
            "max_full_stomach": row["MaxFullStomach"],
            "food_amount": row["FoodAmount"],
            "biological_grade": row["BiologicalGrade"],
            "stamina": row["Stamina"],
            "male_probability": row["MaleProbability"],
            "combi_rank": row["CombiRank"],
            "work_suitability": {
                ws: row[f"WorkSuitability_{ws}"] for ws in WORK_SUITABILITIES
            },
            "skill_set": skill_set,
            "passive_skills": passives,
            "disabled": (old or {}).get("disabled", False),
            "icon": icon_for(key, old),
        }

    # Source rows: all humans, and all monsters except BOSS_ variants (PSP
    # resolves those to the base pal by stripping the prefix at lookup time).
    sources: dict[str, dict] = {}
    for key, row in mon.items():
        if not key.lower().startswith("boss_"):
            sources[key] = row
    sources.update(hum)

    src_map = norm_key_map(sources)
    result = {}
    for key, old in existing.items():
        src_key = src_map.get(key.lower())
        if src_key is None:
            result[key] = old  # no dump source; keep as-is
        else:
            result[key] = make_entry(key, sources[src_key], old)
    known = {k.lower() for k in existing}
    for key in sorted(sources):
        if key.lower() not in known:
            result[key] = make_entry(key, sources[key], None)
    return result


# ---------------------------------------------------------------- active skills

def build_active_skills(dump: Path, existing: dict) -> dict:
    waza = read_rows(dump, "Pal/DataTable/Waza/DT_WazaDataTable.json")
    by_id = {row["WazaType"]: row for row in waza.values()}

    def make_entry(row: dict) -> dict:
        effects = []
        i = 1
        while f"EffectType{i}" in row:
            etype = strip_enum(row[f"EffectType{i}"])
            if etype != "None":
                effects.append({
                    "type": etype,
                    "value": row[f"EffectValue{i}"],
                    "value_ex": row[f"EffectValueEx{i}"],
                })
            i += 1
        return {
            "element": strip_enum(row["Element"]),
            "type": strip_enum(row["Category"]),
            "power": row["Power"],
            "min_range": row["MinRange"],
            "max_range": row["MaxRange"],
            "cool_time": row["CoolTime"],
            "effects": effects,
        }

    result = {}
    for key, old in existing.items():
        result[key] = make_entry(by_id[key]) if key in by_id else old
    for key in sorted(by_id):
        if key not in result:
            result[key] = make_entry(by_id[key])
    return result


# ---------------------------------------------------------------- passive skills

def build_passive_skills(dump: Path, existing: dict) -> dict:
    psv = read_rows(dump, "Pal/DataTable/PassiveSkill/DT_PassiveSkill_Main.json")
    names = read_rows(dump, "L10N/en/Pal/DataTable/Text/DT_SkillNameText_Common.json")
    named = {k[len("PASSIVE_"):] for k in names if k.startswith("PASSIVE_")}
    # Also include unnamed passives that gear or pals reference, so every
    # passive_skills[] id in the other data files resolves.
    items_dt = read_rows(dump, "Pal/DataTable/Item/DT_ItemDataTable.json")
    for row in items_dt.values():
        for f in ("PassiveSkillName", "PassiveSkillName2",
                  "PassiveSkillName3", "PassiveSkillName4"):
            v = strip_enum(row.get(f, "None"))
            if v != "None":
                named.add(v)
    for tbl in ("Character/DT_PalMonsterParameter.json",
                "Character/DT_PalHumanParameter.json"):
        for row in read_rows(dump, "Pal/DataTable/" + tbl).values():
            for f in ("PassiveSkill1", "PassiveSkill2", "PassiveSkill3", "PassiveSkill4"):
                v = strip_enum(row.get(f, "None"))
                if v != "None":
                    named.add(v)

    def make_entry(key: str, row: dict, old: dict | None) -> dict:
        effects = []
        for i in range(1, 5):
            etype = strip_enum(row[f"EffectType{i}"])
            if etype == "no":
                continue
            target = strip_enum(row[f"TargetType{i}"])
            effects.append({
                "type": etype if etype in KNOWN_EFFECT_TYPES else "None",
                "value": row[f"EffectValue{i}"],
                "target": target if target in KNOWN_EFFECT_TARGETS else "None",
            })
        return {
            "rank": row["Rank"],
            "effects": effects,
            "invoke_active_party": row["InvokeActiveOtomo"],
            "invoke_worker": row["InvokeWorker"],
            "invoke_riding": row["InvokeRiding"],
            "invoke_reserve": row["InvokeReserve"],
            "invoke_in_party": row["InvokeInOtomo"],
            "invoke_always": row["InvokeAlways"],
            "invoke_in_base": row["InvokeInBaseCamp"],
            "add_pal": row["AddPal"],
            "add_rare_pal": row["AddRarePal"],
            "add_shot_weapon": row["AddShotWeapon"],
            "add_melee_weapon": row["AddMeleeWeapon"],
            "add_armor": row["AddArmor"],
            "add_accessory": row["AddAccessory"],
            "disabled": (old or {}).get("disabled", False),
        }

    result = {}
    for key, old in existing.items():
        result[key] = make_entry(key, psv[key], old) if key in psv else old
    known = set(existing)
    for key in sorted(named & set(psv)):
        if key not in known:
            result[key] = make_entry(key, psv[key], None)
    return result


# ---------------------------------------------------------------- items

DYNAMIC_TYPES = {"CommonArmor": "armor", "CommonWeapon": "weapon", "PalEgg": "egg"}


def build_items(dump: Path, existing: dict) -> dict:
    dt = read_rows(dump, "Pal/DataTable/Item/DT_ItemDataTable.json")
    icons = read_rows(dump, "Pal/DataTable/Item/DT_ItemIconDataTable.json")
    food = read_rows(dump, "Pal/DataTable/Item/DT_StatusEffectFood.json")
    names = read_rows(dump, "L10N/en/Pal/DataTable/Text/DT_ItemNameText_Common.json")
    named = {k[len("ITEM_NAME_"):].lower() for k in names if k.startswith("ITEM_NAME_")}

    # PSP's `group` is a UI grouping derived from (TypeA, TypeB); learn the
    # mapping from the existing file instead of hard-coding it.
    group_map = {}
    for key, entry in existing.items():
        if key in dt:
            pair = (dt[key]["TypeA"], dt[key]["TypeB"])
            group_map.setdefault(pair, Counter())[entry["group"]] += 1
    group_map = {pair: c.most_common(1)[0][0] for pair, c in group_map.items()}

    def icon_name(key: str, old: dict | None) -> str:
        row = icons.get(key)
        if row:
            asset = row["Icon"]["AssetPathName"]
            return asset.rsplit("/", 1)[-1].split(".")[0].lower()
        return (old or {}).get("icon", f"t_itemicon_{key.lower()}")

    def make_entry(key: str, row: dict, old: dict | None) -> dict:
        type_a = strip_enum(row["TypeA"])
        type_b = strip_enum(row["TypeB"])
        if type_b == "None":
            type_b = "NONE"  # existing file's convention for typeless items
        pair = (row["TypeA"], row["TypeB"])
        entry = {
            "group": group_map.get(pair, (old or {}).get("group", "None")),
            "type_a": type_a,
            "type_b": type_b,
            "rank": row["Rank"],
            "rarity": row["Rarity"],
            "max_stack_count": row["MaxStackCount"],
            "weight": row["Weight"],
            "price": row["Price"],
            "sort_id": row["SortId"],
            "icon": icon_name(key, old),
        }
        dyn_type = DYNAMIC_TYPES.get(row["ItemDynamicClass"])
        if dyn_type:
            passives = [
                strip_enum(row[f])
                for f in ("PassiveSkillName", "PassiveSkillName2",
                          "PassiveSkillName3", "PassiveSkillName4")
                if strip_enum(row[f]) != "None"
            ]
            if dyn_type == "weapon":
                entry["dynamic"] = {"type": "weapon", "durability": row["Durability"]}
                if row["MagazineSize"]:
                    entry["dynamic"]["magazine_size"] = row["MagazineSize"]
                entry["dynamic"]["passive_skills"] = passives
            elif dyn_type == "armor" and row["Durability"]:
                entry["dynamic"] = {
                    "type": "armor",
                    "durability": row["Durability"],
                    "passive_skills": passives,
                }
            else:
                entry["dynamic"] = {"type": dyn_type, "passive_skills": passives}
        if row["PhysicalAttackValue"]:
            entry["damage"] = row["PhysicalAttackValue"]
        if row["PhysicalDefenseValue"]:
            entry["defense"] = row["PhysicalDefenseValue"]
        if row["CorruptionFactor"]:
            entry["corruption_factor"] = row["CorruptionFactor"]
        entry["disabled"] = (old or {}).get("disabled", False)
        if key in food:
            frow = food[key]
            modifiers = []
            for i in (1, 2):
                ftype = strip_enum(frow[f"EffectType{i}"])
                if ftype != "None":
                    modifiers.append({
                        "type": ftype,
                        "value": frow[f"EffectValue{i}"],
                        "interval": frow[f"Interaval{i}"],
                    })
            if modifiers:
                entry["effect"] = {"duration": frow["EffectTime"], "modifiers": modifiers}
        if "effect" not in entry and old and "effect" in old:
            entry["effect"] = old["effect"]
        return entry

    result = {}
    for key, old in existing.items():
        result[key] = make_entry(key, dt[key], old) if key in dt else old
    known = {k.lower() for k in existing}
    # New entries: only player-visible items (named in the l10n text tables);
    # the unnamed remainder are NPC/debug internals PSP has always excluded.
    for key in sorted(dt):
        if key.lower() not in known and key.lower() in named:
            result[key] = make_entry(key, dt[key], None)
    return result


# ---------------------------------------------------------------- buildings

def build_buildings(dump: Path, existing: dict) -> dict:
    dt = read_rows(dump, "Pal/DataTable/MapObject/Building/DT_BuildObjectDataTable.json")
    master = read_rows(dump, "Pal/DataTable/MapObject/DT_MapObjectMasterDataTable.json")
    icons = read_rows(dump, "Pal/DataTable/MapObject/Building/DT_BuildObjectIconDataTable_Common.json")

    def icon_for(key: str, old: dict | None) -> str:
        row = icons.get(key)
        if row:
            asset = row.get("SoftIcon", {}).get("AssetPathName") or ""
            if asset:
                return asset.rsplit("/", 1)[-1].split(".")[0].lower()
        return (old or {}).get("icon") or f"t_icon_buildobject_{key.lower()}"

    def make_entry(key: str, row: dict, old: dict | None) -> dict:
        materials = []
        for i in (1, 2, 3, 4):
            mid = row[f"Material{i}_Id"]
            if mid != "None":
                materials.append({"id": mid, "count": row[f"Material{i}_Count"]})
        m = master.get(row["MapObjectId"], {})
        entry = {
            "type_a": strip_enum(row["TypeA"]),
            "type_b": strip_enum(row["TypeB"]),
            "rank": row["Rank"],
            "required_build_work_amount": row["RequiredBuildWorkAmount"],
            "required_energy_type": strip_enum(row["RequiredEnergyType"]),
            "consume_energy_speed": row["ConsumeEnergySpeed"],
            "materials": materials,
            "material_type": m.get("MaterialType", (old or {}).get("material_type")),
            "material_sub_type": m.get("MaterialSubType", (old or {}).get("material_sub_type")),
            "hp": m.get("Hp", (old or {}).get("hp")),
            "defense": m.get("Defense", (old or {}).get("defense")),
            "deterioration_damage": m.get("DeteriorationDamage", (old or {}).get("deterioration_damage")),
            "icon": icon_for(key, old),
        }
        if (old or {}).get("disabled"):
            entry["disabled"] = True
        return entry

    result = {}
    for key, old in existing.items():
        result[key] = make_entry(key, dt[key], old) if key in dt else old
    known = {k.lower() for k in existing}
    for key in sorted(dt):
        if key.lower() not in known:
            result[key] = make_entry(key, dt[key], None)
    return result


# ---------------------------------------------------------------- exp

def build_exp(dump: Path, existing: dict) -> dict:
    dt = read_rows(dump, "Pal/DataTable/Exp/DT_PalExpTable.json")
    fields = ["DropEXP", "NextEXP", "PalNextEXP", "TotalEXP", "PalTotalEXP"]
    result = {}
    for key, old in existing.items():
        row = dt.get(key)
        result[key] = {f: row[f] for f in fields} if row else old
    for key in dt:
        if key not in result:
            result[key] = {f: dt[key][f] for f in fields}
    return result


# ---------------------------------------------------------------- l10n

# base file -> (name table, name prefix, desc table, desc prefix)
# {lang} is replaced per language. Tables are per-key merged.
L10N_SPEC = {
    "pals": [
        ("L10N/{lang}/Pal/DataTable/Text/DT_PalNameText_Common.json", "PAL_NAME_",
         "L10N/{lang}/Pal/DataTable/Text/DT_PalLongDescriptionText.json", "PAL_LONG_DESC_"),
        ("L10N/{lang}/Pal/DataTable/Text/DT_HumanNameText_Common.json", "NAME_",
         None, None),
    ],
    "active_skills": [
        ("L10N/{lang}/Pal/DataTable/Text/DT_SkillNameText_Common.json", "ACTION_SKILL_",
         "L10N/{lang}/Pal/DataTable/Text/DT_SkillDescText_Common.json", "ACTION_SKILL_"),
    ],
    "passive_skills": [
        ("L10N/{lang}/Pal/DataTable/Text/DT_SkillNameText_Common.json", "PASSIVE_",
         "L10N/{lang}/Pal/DataTable/Text/DT_SkillDescText_Common.json", "PASSIVE_"),
    ],
    "items": [
        ("L10N/{lang}/Pal/DataTable/Text/DT_ItemNameText_Common.json", "ITEM_NAME_",
         "L10N/{lang}/Pal/DataTable/Text/DT_ItemDescriptionText_Common.json", "ITEM_DESC_"),
    ],
    "buildings": [
        ("L10N/{lang}/Pal/DataTable/Text/DT_MapObjectNameText_Common.json", "MAPOBJECT_NAME_",
         "L10N/{lang}/Pal/DataTable/Text/DT_BuildObjectDescText_Common.json", "BUILDOBJECT_DESC_"),
    ],
    # Element and work-suitability display names live in the shared UI table;
    # the PSP keys match the game key suffixes 1:1.
    "elements": [
        ("L10N/{lang}/Pal/DataTable/Text/DT_UI_Common_Text_Common.json",
         "COMMON_ELEMENT_NAME_", None, None),
    ],
    "work_suitability": [
        ("L10N/{lang}/Pal/DataTable/Text/DT_UI_Common_Text_Common.json",
         "COMMON_WORK_SUITABILITY_", None, None),
    ],
}

# Files whose l10n text key is carried in a DataTable row field rather than
# derived from the entry key: base file -> (DT path, text-key field, text table).
L10N_INDIRECT = {
    "technologies": (
        "Pal/DataTable/Technology/DT_TechnologyRecipeUnlock.json", "Name", "Description",
        "L10N/{lang}/Pal/DataTable/Text/DT_TechnologyNameText_Common.json",
        "L10N/{lang}/Pal/DataTable/Text/DT_TechnologyDescText_Common.json",
    ),
    "lab_research": (
        "Pal/DataTable/Lab/DT_LabResearchDataTable.json", "TextId", None,
        "L10N/{lang}/Pal/DataTable/Text/DT_LabResearchText.json",
        "L10N/{lang}/Pal/DataTable/Text/DT_LabResearchText.json",
    ),
}


# "Previously committed translation" source. With PSP_L10N_BASELINE set to a
# git ref (e.g. the merge-base with upstream), fallbacks only trust
# translations that existed at that ref — text this branch itself generated
# earlier is regenerated from the dump instead of being recycled, so a bad
# earlier run can't keep contaminating later ones.
_BASELINE_REF = os.environ.get("PSP_L10N_BASELINE")
_BASELINE_CACHE: dict = {}


def _load_baseline(lang: str, base_name: str) -> dict | None:
    key = (lang, base_name)
    if key not in _BASELINE_CACHE:
        rel = f"data/json/l10n/{lang}/{base_name}.json"
        proc = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "show", f"{_BASELINE_REF}:{rel}"],
            capture_output=True, text=True)
        _BASELINE_CACHE[key] = json.loads(proc.stdout) if proc.returncode == 0 else {}
    return _BASELINE_CACHE[key]


def _load_existing(lang: str, base_name: str) -> dict:
    """Previous l10n entries for a language; a language that mirrors another
    (id-id -> id) also inherits the source's hand-maintained entries."""
    if _BASELINE_REF:
        own = _load_baseline(lang, base_name)
        src = LANG_SOURCE_OVERRIDES.get(lang)
        if src:
            merged = dict(own)
            for key, entry in _load_baseline(src, base_name).items():
                if _clean(entry.get("localized_name")):
                    merged[key] = entry
            return merged
        return own
    own_path = L10N_DIR / lang / f"{base_name}.json"
    own = load_psp_json(own_path) if own_path.exists() else {}
    src = LANG_SOURCE_OVERRIDES.get(lang)
    if src:
        src_path = L10N_DIR / src / f"{base_name}.json"
        if src_path.exists():
            merged = dict(own)
            for key, entry in load_psp_json(src_path).items():
                if _clean(entry.get("localized_name")):
                    merged[key] = entry
            return merged
    return own


def build_l10n_indirect(dump: Path, base_name: str, base_keys: list[str]) -> dict:
    dt_path, name_field, desc_field, name_tbl, desc_tbl = L10N_INDIRECT[base_name]
    dt = read_rows(dump, dt_path)
    text_dir = "L10N/{lang}/Pal/DataTable/Text/"
    langs = sorted(d.name for d in L10N_DIR.iterdir() if d.is_dir())
    en_resolve = make_markup_resolver(dump, "en")
    result = {}
    for lang in langs:
        src = LANG_SOURCE_OVERRIDES.get(lang, lang)
        resolve = en_resolve if src == "en" else make_markup_resolver(dump, src)

        def texts(rel, lang_code):
            try:
                return {k.lower(): localized_string(v)
                        for k, v in read_rows(dump, rel.format(lang=lang_code)).items()}
            except FileNotFoundError:
                return {}

        names, descs = texts(name_tbl, src), texts(desc_tbl, src)
        en_names, en_descs = texts(name_tbl, "en"), texts(desc_tbl, "en")
        # Technologies that unlock a recipe display the unlocked item's /
        # building's name in-game when they have no own name text.
        item_names = read_text_table(dump, text_dir.format(lang=src) + "DT_ItemNameText_Common.json", "ITEM_NAME_")
        en_item_names = read_text_table(dump, text_dir.format(lang="en") + "DT_ItemNameText_Common.json", "ITEM_NAME_")
        build_names = read_text_table(dump, text_dir.format(lang=src) + "DT_MapObjectNameText_Common.json", "MAPOBJECT_NAME_")
        en_build_names = read_text_table(dump, text_dir.format(lang="en") + "DT_MapObjectNameText_Common.json", "MAPOBJECT_NAME_")

        def unlock_fallback(row: dict, items_tbl: dict, builds_tbl: dict) -> str | None:
            for rid in row.get("UnlockItemRecipes") or []:
                v = items_tbl.get(rid.lower())
                if v:
                    return v
            for bid in row.get("UnlockBuildObjects") or []:
                v = builds_tbl.get(bid.lower())
                if v:
                    return v
            return None

        def pick(candidates):
            for value, resolver in candidates:
                if value is not None:
                    return resolver(value)
            return None

        existing = _load_existing(lang, base_name)
        lang_out = {}
        for key in base_keys:
            row = dt.get(key)
            old = existing.get(key)
            old_name = _clean((old or {}).get("localized_name"))
            old_desc = _clean((old or {}).get("description"))
            if row is None:
                if old_name:
                    lang_out[key] = {"localized_name": resolve(old_name),
                                     "description": resolve(old_desc)}
                continue
            nk = row[name_field].lower()
            if desc_field:
                dk = row[desc_field].lower()
            else:
                dk = nk.replace("name_", "desc_", 1)
            # Language-correct preference: this language's own text (direct,
            # then via the unlocked item/building the game displays), then
            # the previously committed translation, then English — each
            # candidate resolved in the language of the text it came from.
            # A committed raw code name still beats the full enum key.
            unlock_own = unlock_fallback(row, item_names, build_names) \
                if base_name == "technologies" else None
            unlock_en = unlock_fallback(row, en_item_names, en_build_names) \
                if base_name == "technologies" else None
            name = pick([
                (names.get(nk), resolve),
                (unlock_own, resolve),
                (_clean(old_name, key), resolve),
                (en_names.get(nk), en_resolve),
                (unlock_en, en_resolve),
                (old_name, resolve),
            ])
            desc = pick([
                (descs.get(dk), resolve),
                (old_desc, resolve),
                (en_descs.get(dk), en_resolve),
            ])
            lang_out[key] = {
                "localized_name": name if name is not None else key,
                "description": desc,
            }
        result[lang] = lang_out
    return result


_ITEM_OVERRIDES: dict = {}


def item_overrides(dump: Path) -> tuple[dict, dict]:
    """Items may carry OverrideName/OverrideDescription pointing at a
    different text key (e.g. GrapplingGun -> ITEM_NAME_GrapplingGun_1).
    Returns {item_code_lower: override_code_lower} for names and descs."""
    if not _ITEM_OVERRIDES:
        dt = read_rows(dump, "Pal/DataTable/Item/DT_ItemDataTable.json")
        names, descs = {}, {}
        for code, row in dt.items():
            ov = row.get("OverrideName", "None")
            if ov != "None" and ov.startswith("ITEM_NAME_"):
                names[code.lower()] = ov[len("ITEM_NAME_"):].lower()
            ov = row.get("OverrideDescription", "None")
            if ov != "None" and ov.startswith("ITEM_DESC_"):
                descs[code.lower()] = ov[len("ITEM_DESC_"):].lower()
        _ITEM_OVERRIDES["names"] = names
        _ITEM_OVERRIDES["descs"] = descs
    return _ITEM_OVERRIDES["names"], _ITEM_OVERRIDES["descs"]


# Game text embeds UE RichText markup. Reference tags (<itemName id=|X|/>)
# are resolved against the same language's text tables; style tags
# (<Status_Up>...</>) are stripped keeping their content; icon tags dropped.
# Some shipped rows close the id with an apostrophe instead of the second
# pipe ("<itemName id=|Head004_5'/>", a game-side typo) — accept both.
_REF_TAG = re.compile(r"<(\w+)\s+id=\|([^|'>]*)['|][^>]*/>")
_ICON_TAG = re.compile(r"<(?:img|keyGuideIcon)\s+[^>]*/>")
_STYLE_TAG = re.compile(r"</?\w[^>]*>|</>")


_EFFECT_TOKEN = re.compile(r"\{EffectValue(\d)\}")


def _subst_effect_values(text: str | None, row) -> str | None:
    """Passive descriptions embed {EffectValueN} tokens that the game fills
    from DT_PassiveSkill_Main at runtime; bake the row's values in."""
    if not text or row is None or "{EffectValue" not in text:
        return text

    def repl(m):
        v = row.get(f"EffectValue{m.group(1)}")
        if v is None:
            return m.group(0)
        return f"{v:g}" if isinstance(v, float) else str(v)

    return _EFFECT_TOKEN.sub(repl, text)


def _pipes_to_quotes(text: str, lang: str) -> str:
    """The game marks quoted phrases with pipes ("|word|"), but some locales
    mix them with real CJK quote brackets ("「word|"). Convert pipes to real
    quotes, closing whatever opener (CJK bracket or pipe) they pair with."""
    if "|" not in text:
        return text
    pipe_open, pipe_close = ("「", "」") if lang.startswith("zh") else ('"', '"')
    pairs = {"「": "」", "『": "』", "“": "”", "|": pipe_close}
    closers = {"」", "』", "”"}
    out: list[str] = []
    stack: list[str] = []
    for ch in text:
        if ch == "|":
            if stack:
                out.append(pairs[stack.pop()])
            else:
                stack.append("|")
                out.append(pipe_open)
        elif ch in pairs:
            stack.append(ch)
            out.append(ch)
        elif ch in closers:
            if stack and pairs.get(stack[-1]) == ch:
                stack.pop()
            out.append(ch)
        else:
            out.append(ch)
    return "".join(out)


def make_markup_resolver(dump: Path, lang: str):
    tables = {}

    def table(rel: str, prefix: str, lang_code: str) -> dict:
        cache_key = (rel, prefix, lang_code)
        if cache_key not in tables:
            tables[cache_key] = read_text_table(dump, rel.format(lang=lang_code), prefix)
        return tables[cache_key]

    def lookup_in(tag: str, ref_id: str, lang_code: str) -> str | None:
        text_dir = "L10N/{lang}/Pal/DataTable/Text/"
        if tag == "itemname":
            item_tbl = table(text_dir + "DT_ItemNameText_Common.json", "ITEM_NAME_", lang_code)
            ov = item_overrides(dump)[0].get(ref_id.lower())
            return (item_tbl.get(ov) if ov else None) or item_tbl.get(ref_id.lower())
        if tag == "charactername":
            return (table(text_dir + "DT_PalNameText_Common.json", "PAL_NAME_", lang_code).get(ref_id.lower())
                    or table(text_dir + "DT_HumanNameText_Common.json", "NAME_", lang_code).get(ref_id.lower()))
        if tag == "mapobjectname":
            return table(text_dir + "DT_MapObjectNameText_Common.json", "MAPOBJECT_NAME_", lang_code).get(ref_id.lower())
        if tag == "activeskillname":
            return table(text_dir + "DT_SkillNameText_Common.json", "ACTION_SKILL_", lang_code).get(ref_id.lower())
        if tag == "uicommon":
            return table(text_dir + "DT_UI_Common_Text_Common.json", "", lang_code).get(ref_id.lower())
        return None

    def lookup(tag: str, ref_id: str) -> str | None:
        tag = tag.lower()
        # own language first; unauthored rows fall back to the English name
        # so a display name never degrades to a raw code id.
        value = lookup_in(tag, ref_id, lang)
        if value is None and lang != "en":
            value = lookup_in(tag, ref_id, "en")
        return value

    def resolve(text: str | None) -> str | None:
        if not text:
            return text
        if "<" in text:
            # Icon tags (<img id=|X|/>) are visual-only and must be stripped
            # before the reference-tag loop: _REF_TAG's shape also matches
            # them, and since "img" isn't a recognized lookup() tag type,
            # leaving them for that pass leaks the raw icon id as text
            # (e.g. "<img id=|ElemIcon_Fire|/>Fuego" -> "ElemIcon_FireFuego").
            text = _ICON_TAG.sub("", text)
            for _ in range(3):  # referenced names may themselves contain tags
                new = _REF_TAG.sub(lambda m: lookup(m.group(1), m.group(2)) or m.group(2), text)
                if new == text:
                    break
                text = new
            text = _STYLE_TAG.sub("", text)
        text = _pipes_to_quotes(text, lang)
        return re.sub(r"  +", " ", text).strip()

    return resolve


def read_text_table(dump: Path, rel: str, prefix: str) -> dict:
    try:
        rows = read_rows(dump, rel)
    except FileNotFoundError:
        return {}
    out = {}
    for k, v in rows.items():
        if k.startswith(prefix):
            code = k[len(prefix):]
            value = localized_string(v)
            # Another unauthored-row pattern: the text is the raw internal
            # code itself ("Antibiotic_Good"). Natural display names never
            # look like code identifiers.
            if value == code and re.search(r"[_0-9]|[a-z][A-Z]", code):
                value = None
            out[code.lower()] = value
    return out


# Hand corrections for typos in the game's own text, keyed by the exact
# broken value so each fix deactivates automatically once the game ships
# corrected text. (lang, base, key, field) -> (broken, fixed). The lang key
# is the SOURCE language (LANG_SOURCE_OVERRIDES applied), so mirror locales
# (id-id) inherit their source's fixes automatically.
# Committed strings (any language) that must never survive as fallbacks.
_BAD_COMMITTED_PREFIXES = (
    "When in inventory, unlocks recipe for Head017 2",
)

HAND_FIXES = {
    ("ko", "items", "Blueprint_Head017_5", "localized_name"):
        ("크레메ーオ 모자 디자인 도면 5", "캐티메이지 모자 설계도 5"),
    # game text typo: Japanese "の" in the Thai string (sibling _1 uses "ของ")
    ("th", "passive_skills", "WorkSuitabilityAddRank_MonsterFarm_2", "description"):
        ("ความถนัดงาน+2のฟาร์ม", "ความถนัดงาน+2ของฟาร์ม"),
    # es-MX has no native work_suitability text for this key anywhere in the
    # game data, so it falls back to English; use the es (Spanish) locale's
    # translation instead, since es-MX/es share the language.
    ("es-MX", "work_suitability", "OilExtraction", "localized_name"):
        ("Crude Oil Extraction", "Extracción de petróleo crudo"),
    # id game text garbles the "4x mid-air dash" description (sibling
    # AirDash3 uses the correct "melompat 3 kali di udara" pattern).
    ("id", "items", "Accessory_AirDash4", "description"): (
        "Aksesori yang saat dipakai membuat pemain jadi bisa berlari di 4 kali udara.",
        "Aksesori yang saat dipakai membuat pemain bisa melakukan dash di udara sebanyak 4 kali.",
    ),
    # game text has a stray katakana interpunct (・) nowhere else used in the
    # Thai localization; every other Thai skill name is space-separated.
    ("th", "active_skills", "EPalWazaID::Unique_GrassMinotaur_BullRush", "localized_name"):
        ("พุ่งชน・คลั่ง", "พุ่งชน คลั่ง"),
    # fr is the only locale (of 17) whose text equals the raw key "Vampire"
    # instead of a real translation; en/es/it/etc. all use the adjectival
    # form ("Vampiric"/"Vampírico"), so match that convention in French.
    ("fr", "passive_skills", "Vampire", "localized_name"):
        ("Vampire", "Vampirique"),
    # zh-Hans/zh-Hant game text is a literal untranslated placeholder
    # ("zh Hans/Hant Text"), which dt_reader treats as missing, so the value
    # arriving here is the English fallback; reuse PalEgg_MutationPal_05's
    # translation since both keys share the identical EN name.
    ("zh-Hans", "items", "PalEgg_MutationPal", "localized_name"):
        ("Huge Mutated Egg", "巨大突变帕鲁蛋"),
    ("zh-Hant", "items", "PalEgg_MutationPal", "localized_name"):
        ("Huge Mutated Egg", "巨大突變帕魯蛋"),
}


def _hand_fix(lang: str, base: str, key: str, field: str, value):
    fix = HAND_FIXES.get((lang, base, key, field))
    if fix and value == fix[0]:
        return fix[1]
    return value


def build_l10n(dump: Path, base_name: str, base_keys: list[str]) -> dict:
    """Returns {lang: {key: {localized_name, description}}} for all PSP langs."""
    langs = sorted(d.name for d in L10N_DIR.iterdir() if d.is_dir())
    spec = L10N_SPEC[base_name]

    def tables_for(lang: str):
        names, descs = {}, {}
        for name_rel, name_pre, desc_rel, desc_pre in spec:
            names.update(read_text_table(dump, name_rel.format(lang=lang), name_pre))
            if desc_rel:
                descs.update(read_text_table(dump, desc_rel.format(lang=lang), desc_pre))
        return names, descs

    en_names, en_descs = tables_for("en")
    name_ov, desc_ov = item_overrides(dump) if base_name == "items" else ({}, {})
    en_existing = _load_existing("en", base_name)
    en_resolve = make_markup_resolver(dump, "en")
    effect_rows = (read_rows(dump, "Pal/DataTable/PassiveSkill/DT_PassiveSkill_Main.json")
                   if base_name == "passive_skills" else {})
    result = {}
    for lang in langs:
        src = LANG_SOURCE_OVERRIDES.get(lang, lang)
        names, descs = tables_for(src) if src != "en" else (en_names, en_descs)
        # embedded references must be resolved in the language of the text
        # they sit in, or an English fallback sentence ends up with a
        # translated facility name spliced into it (and vice versa).
        resolve = en_resolve if src == "en" else make_markup_resolver(dump, src)

        def pick(candidates):
            for value, resolver in candidates:
                if value is not None:
                    return resolver(value)
            return None

        existing = _load_existing(lang, base_name)
        lang_out = {}
        for key in base_keys:
            lk = key.split("::")[-1].lower()
            nk = name_ov.get(lk, lk)
            dk = desc_ov.get(lk, lk)
            old = existing.get(key)
            old_name_raw = _clean((old or {}).get("localized_name"))
            old_name = _clean(old_name_raw, key)
            old_desc = _clean((old or {}).get("description"))
            old_en = en_existing.get(key) or {}
            # Prefer language-correct text: this language's game text, then
            # the previously committed translation, then English game text,
            # then the committed English translation (hand-curated names for
            # rows the game text tables never authored). A committed value
            # that is just the raw code name loses to those, but still beats
            # the full enum key.
            # Some override targets are bogus (Shield_05 -> ITEM_NAME_PV_ITEMS),
            # so a failed override lookup falls back to the item's own key.
            name = pick([
                (names.get(nk) or names.get(lk), resolve),
                (old_name, resolve),
                (en_names.get(nk) or en_names.get(lk), en_resolve),
                (_clean(old_en.get("localized_name"), key), en_resolve),
                (old_name_raw, resolve),
            ])
            desc = pick([
                (descs.get(dk) or descs.get(lk), resolve),
                (old_desc, resolve),
                (en_descs.get(dk) or en_descs.get(lk), en_resolve),
                (_clean(old_en.get("description")), en_resolve),
            ])
            lang_out[key] = {
                # unauthored rows fall back to the bare code name, never the
                # full enum key ("EPalWazaID::X" -> "X")
                "localized_name": _hand_fix(src, base_name, key, "localized_name",
                                            name if name is not None
                                            else key.split("::")[-1]),
                "description": _hand_fix(src, base_name, key, "description",
                                          _subst_effect_values(desc, effect_rows.get(key))),
            }
        result[lang] = lang_out
    return result


# ---------------------------------------------------------------- diff / io

def diff_summary(name: str, old: dict, new: dict) -> str:
    added = [k for k in new if k not in old]
    removed = [k for k in old if k not in new]
    changed = [k for k in new if k in old and new[k] != old[k]]
    lines = [f"[{name}] {len(old)} -> {len(new)} entries: "
             f"+{len(added)} added, ~{len(changed)} changed, -{len(removed)} removed"]
    if added:
        lines.append(f"  added: {', '.join(added[:20])}{' ...' if len(added) > 20 else ''}")
    if removed:
        lines.append(f"  REMOVED: {', '.join(removed[:20])}")
    return "\n".join(lines)


BUILDERS = {
    "pals": build_pals,
    "active_skills": build_active_skills,
    "passive_skills": build_passive_skills,
    "items": build_items,
    "buildings": build_buildings,
    "exp": build_exp,
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dump-dir", required=True, type=Path,
                        help="Path to <extract>/Pal/Content of the PAK dump")
    parser.add_argument("--only", default=None,
                        help="Comma-separated subset of: "
                             + ",".join(BUILDERS) + ",l10n")
    parser.add_argument("--check", action="store_true",
                        help="Report the diff without writing anything")
    args = parser.parse_args()

    targets = args.only.split(",") if args.only else list(BUILDERS) + ["l10n"]
    dump = args.dump_dir
    built = {}

    for name in targets:
        if name == "l10n":
            continue
        existing = load_psp_json(DATA_DIR / f"{name}.json")
        new = BUILDERS[name](dump, existing)
        built[name] = new
        print(diff_summary(name, existing, new))
        if not args.check:
            save_psp_json(DATA_DIR / f"{name}.json", new)

    if "l10n" in targets:
        for base_name, builder in [(n, build_l10n) for n in L10N_SPEC] + \
                [(n, build_l10n_indirect) for n in L10N_INDIRECT]:
            base_path = DATA_DIR / f"{base_name}.json"
            if base_path.exists():
                base = built.get(base_name) or load_psp_json(base_path)
            else:
                # no base data file (e.g. work_suitability): the English l10n
                # file is the key authority instead.
                base = load_psp_json(L10N_DIR / "en" / f"{base_name}.json")
            per_lang = builder(dump, base_name, list(base))
            for lang, data in per_lang.items():
                path = L10N_DIR / lang / f"{base_name}.json"
                old = load_psp_json(path) if path.exists() else {}
                if old != data:
                    print(diff_summary(f"l10n/{lang}/{base_name}", old, data))
                if not args.check:
                    save_psp_json(path, data)

    if args.check:
        print("\n--check: no files were written")


if __name__ == "__main__":
    main()
