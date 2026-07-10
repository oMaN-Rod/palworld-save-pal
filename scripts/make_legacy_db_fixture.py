# scripts/make_legacy_db_fixture.py
"""Build a deterministic legacy psp.db fixture (SQLModel schema) for Rust importer tests.

Run from the repo root:  uv run python scripts/make_legacy_db_fixture.py
"""

import os
import sys
from datetime import datetime
from uuid import UUID

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from sqlmodel import Session, SQLModel, create_engine

from palworld_save_pal.db.models.server_models import ServerModel
from palworld_save_pal.db.models.settings_model import SettingsModel
from palworld_save_pal.db.models.ups_models import (
    UPSCollectionModel,
    UPSPalModel,
    UPSStatsModel,
    UPSTagModel,
    UPSTransferLogModel,
)
from palworld_save_pal.editor.preset_profile import PalPreset, PresetProfile
from palworld_save_pal.game.pal_objects import PalGender

OUTPUT_PATH = os.path.join("rust", "psp-db", "tests", "fixtures", "legacy_psp.db")
FIXED_TIME = datetime(2026, 1, 2, 3, 4, 5, 123456)

VALID_PAL_DATA = {
    "instance_id": "11111111-1111-1111-1111-111111111111",
    "owner_uid": None,
    "character_id": "SheepBall",
    "is_lucky": False,
    "is_boss": False,
    "gender": "Female",
    "rank_hp": 1,
    "rank_attack": 2,
    "rank_defense": 3,
    "rank_craftspeed": 4,
    "talent_hp": 50,
    "talent_shot": 60,
    "talent_defense": 70,
    "rank": 1,
    "level": 12,
    "exp": 3450,
    "nickname": "Fluffy",
    "is_tower": False,
    "storage_id": "22222222-2222-2222-2222-222222222222",
    "stomach": 100.0,
    "storage_slot": 3,
    "learned_skills": [],
    "active_skills": ["EPalWazaID::Unique_SheepBall_Roll"],
    "passive_skills": ["Rare"],
    "hp": 5000,
    "max_hp": 5000,
    "group_id": None,
    "sanity": 100.0,
    "work_suitability": {"Handcraft": 1},
    "is_sick": False,
    "friendship_point": 0,
    "character_key": "sheepball",
}

SECOND_PAL_DATA = dict(VALID_PAL_DATA)
SECOND_PAL_DATA.update(
    {
        "instance_id": "33333333-3333-3333-3333-333333333333",
        "character_id": "BOSS_Kitsunebi",
        "is_boss": True,
        "nickname": None,
        "level": 40,
        "character_key": "kitsunebi",
    }
)

# character_id is an int and level is a string: PalDto validation must reject this row.
BROKEN_PAL_DATA = {"character_id": 12345, "level": "not-a-number"}


def main() -> None:
    os.makedirs(os.path.dirname(OUTPUT_PATH), exist_ok=True)
    if os.path.exists(OUTPUT_PATH):
        os.remove(OUTPUT_PATH)

    engine = create_engine(f"sqlite:///{OUTPUT_PATH}")
    SQLModel.metadata.create_all(engine)

    with Session(engine) as session:
        session.add(
            SettingsModel(
                id=1,
                language="fr",
                save_dir="C:/legacy/saves",
                clone_prefix="[c]",
                new_pal_prefix="[n]",
                debug_mode=True,
                cheat_mode=True,
            )
        )

        pal_preset = PalPreset(
            id="aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            lock=True,
            lock_element=False,
            element="Fire",
            character_id="Kitsunebi",
            is_lucky=False,
            is_boss=True,
            gender=PalGender.FEMALE,
            rank_hp=10,
            rank_attack=10,
            rank_defense=10,
            rank_craftspeed=10,
            talent_hp=100,
            talent_shot=100,
            talent_defense=100,
            rank=5,
            level=60,
            exp=0,
            learned_skills=[],
            active_skills=["EPalWazaID::FireBall"],
            passive_skills=["Legend"],
            sanity=100.0,
            work_suitability={"EmitFlame": 4},
            nickname="MaxFox",
            filtered_nickname="MaxFox",
            stomach=150.0,
            hp=10000,
            friendship_point=42,
        )
        session.add(pal_preset)
        session.flush()
        session.add(
            PresetProfile(
                id="bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                name="Max Fox",
                type="pal_preset",
                pal_preset_id=pal_preset.id,
            )
        )
        session.add(
            PresetProfile(
                id="cccccccc-cccc-cccc-cccc-cccccccccccc",
                name="Starter Kit",
                type="inventory",
                common_container=[{"static_id": "Wood", "count": 999, "slot_index": 0}],
            )
        )

        favorites = UPSCollectionModel(
            name="Favorites",
            description="best pals",
            color="#ff0000",
            created_at=FIXED_TIME,
            updated_at=FIXED_TIME,
        )
        session.add(favorites)
        session.add(
            UPSCollectionModel(name="Breeding", created_at=FIXED_TIME, updated_at=FIXED_TIME)
        )
        session.flush()

        session.add(
            UPSTagModel(name="shiny", color="#00ff00", created_at=FIXED_TIME, updated_at=FIXED_TIME)
        )
        session.add(UPSTagModel(name="trade", created_at=FIXED_TIME, updated_at=FIXED_TIME))

        session.add(
            UPSPalModel(
                instance_id=UUID("44444444-4444-4444-4444-444444444444"),
                character_id="SheepBall",
                nickname="Fluffy",
                level=12,
                pal_data=VALID_PAL_DATA,
                source_save_file="MyWorld",
                source_player_uid=UUID("55555555-5555-5555-5555-555555555555"),
                source_player_name="Omar",
                source_storage_type="pal_box",
                source_storage_slot=3,
                collection_id=favorites.id,
                tags=["shiny"],
                notes="first import",
                created_at=FIXED_TIME,
                updated_at=FIXED_TIME,
            )
        )
        session.add(
            UPSPalModel(
                instance_id=UUID("66666666-6666-6666-6666-666666666666"),
                character_id="BOSS_Kitsunebi",
                nickname=None,
                level=40,
                pal_data=SECOND_PAL_DATA,
                tags=[],
                created_at=FIXED_TIME,
                updated_at=FIXED_TIME,
            )
        )
        session.add(
            UPSPalModel(
                instance_id=UUID("77777777-7777-7777-7777-777777777777"),
                character_id="Corrupt",
                nickname="Broken",
                level=1,
                pal_data=BROKEN_PAL_DATA,
                tags=[],
                created_at=FIXED_TIME,
                updated_at=FIXED_TIME,
            )
        )
        session.flush()

        session.add(
            UPSStatsModel(
                id=1,
                total_pals=3,
                total_collections=2,
                total_tags=2,
                element_distribution='{"Neutral":1,"Fire":1}',
                alpha_count=1,
                last_updated=FIXED_TIME,
            )
        )
        session.add(
            UPSTransferLogModel(
                pal_id=1,
                operation_type="import",
                source_type="pal_box",
                destination_type="ups",
                save_file_name="MyWorld",
                player_name="Omar",
                player_uid=UUID("55555555-5555-5555-5555-555555555555"),
                success=True,
                timestamp=FIXED_TIME,
            )
        )

        session.add(
            ServerModel(
                name="My Server",
                container_name="psp-server-1",
                env_vars={"COMMUNITY": "False"},
                created_at=FIXED_TIME,
                updated_at=FIXED_TIME,
            )
        )

        session.commit()

    print(f"wrote {OUTPUT_PATH}")


if __name__ == "__main__":
    main()
