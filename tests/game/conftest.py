"""Shared fixtures for game integration tests using real save files."""

import asyncio
from pathlib import Path
from uuid import UUID

import pytest

from palworld_save_pal.game.save_manager import SaveManager
from palworld_save_pal.utils.file_manager import FileManager

FIXTURES_DIR = Path(__file__).parent.parent / "fixtures" / "saves"
WORLD1_DIR = FIXTURES_DIR / "world1"
WORLD2_DIR = FIXTURES_DIR / "world2"
GPS_FILE = FIXTURES_DIR / "GlobalPalStorage.sav"

PLAYER_O_UID = UUID("8c2f1930-0000-0000-0000-000000000000")
PLAYER_SKY_UID = UUID("43797f87-0000-0000-0000-000000000000")


async def _noop(msg):
    pass


def _load_save_manager(world_dir: Path) -> SaveManager:
    with open(world_dir / "Level.sav", "rb") as f:
        level_data = f.read()

    level_meta_data = None
    meta_path = world_dir / "LevelMeta.sav"
    if meta_path.exists():
        with open(meta_path, "rb") as f:
            level_meta_data = f.read()

    player_saves = FileManager.get_player_saves(str(world_dir / "Players"))

    sm = SaveManager()
    loop = asyncio.new_event_loop()
    try:
        loop.run_until_complete(
            sm.load_sav_files(level_data, player_saves, level_meta_data, ws_callback=_noop)
        )
    finally:
        loop.close()
    return sm


@pytest.fixture(scope="session")
def event_loop():
    loop = asyncio.new_event_loop()
    yield loop
    loop.close()


# NOTE: Session-scoped fixtures are read-only. Do NOT mutate these in tests.
# Use fresh_save_manager for any test that modifies state.


@pytest.fixture(scope="session")
def save_manager_world1(event_loop) -> SaveManager:
    return _load_save_manager(WORLD1_DIR)


@pytest.fixture(scope="session")
def save_manager_world2(event_loop) -> SaveManager:
    return _load_save_manager(WORLD2_DIR)


@pytest.fixture
def fresh_save_manager() -> SaveManager:
    """A fresh (non-cached) SaveManager for tests that mutate state."""
    return _load_save_manager(WORLD1_DIR)


@pytest.fixture
def fresh_save_manager_world2() -> SaveManager:
    """A fresh (non-cached) SaveManager for world2 mutation tests."""
    return _load_save_manager(WORLD2_DIR)
