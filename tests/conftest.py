import json
import os
import tempfile
from pathlib import Path
from uuid import UUID, uuid4

import pytest
from sqlalchemy import create_engine
from sqlmodel import Session, SQLModel

from palworld_save_pal.game.enum import PalGender, WorkSuitability


# ---------------------------------------------------------------------------
# Database fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def db_engine():
    engine = create_engine("sqlite:///:memory:")
    SQLModel.metadata.create_all(engine)
    yield engine
    engine.dispose()


@pytest.fixture
def db_session(db_engine):
    with Session(db_engine) as session:
        yield session


# ---------------------------------------------------------------------------
# Temporary file fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def tmp_json_file(tmp_path):
    path = tmp_path / "test.json"
    path.write_text("{}", encoding="utf-8")
    return str(path)


# ---------------------------------------------------------------------------
# UUID helpers
# ---------------------------------------------------------------------------

EMPTY_UUID = UUID("00000000-0000-0000-0000-000000000000")
TEST_UUID_1 = UUID("12345678-1234-1234-1234-123456789abc")
TEST_UUID_2 = UUID("abcdef01-abcd-abcd-abcd-abcdef012345")
TEST_UUID_3 = UUID("11111111-2222-3333-4444-555555555555")


# ---------------------------------------------------------------------------
# Mock GVAS data fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def mock_pal_save_parameter():
    """A minimal PalSaveParameter dict structure."""
    from palworld_save_pal.game.pal_objects import PalObjects

    return PalObjects.PalSaveParameter(
        character_id="Lambball",
        instance_id=TEST_UUID_1,
        owner_uid=TEST_UUID_2,
        container_id=TEST_UUID_3,
        slot_idx=0,
        group_id=TEST_UUID_2,
        nickname="TestPal",
        active_skills=["EPalWazaID::AirCanon"],
        passive_skills=["Legend"],
        gender=PalGender.MALE,
    )


# ---------------------------------------------------------------------------
# Save file fixture paths
# ---------------------------------------------------------------------------

FIXTURES_DIR = Path(__file__).parent / "fixtures"
SAVES_DIR = FIXTURES_DIR / "saves"
