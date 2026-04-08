"""Integration tests for GPS (Global Pal Storage) loading."""

from pathlib import Path

import pytest

from palworld_save_pal.game.pal import Pal
from tests.game.conftest import GPS_FILE, _noop


class TestLoadGPS:
    def test_load_gps(self, fresh_save_manager):
        with open(GPS_FILE, "rb") as f:
            gps_data = f.read()

        gps_pals = fresh_save_manager.load_gps(gps_data)
        assert gps_pals is not None
        assert len(gps_pals) > 0

    def test_gps_pals_are_pals(self, fresh_save_manager):
        with open(GPS_FILE, "rb") as f:
            gps_data = f.read()

        gps_pals = fresh_save_manager.load_gps(gps_data)
        for idx, pal in gps_pals.items():
            assert isinstance(pal, Pal)
            assert pal.character_id is not None
            assert pal.level >= 1

    def test_gps_pals_count(self, fresh_save_manager):
        with open(GPS_FILE, "rb") as f:
            gps_data = f.read()

        gps_pals = fresh_save_manager.load_gps(gps_data)
        assert len(gps_pals) == 6

    def test_get_gps_returns_loaded(self, fresh_save_manager):
        with open(GPS_FILE, "rb") as f:
            gps_data = f.read()

        fresh_save_manager.load_gps(gps_data)
        result = fresh_save_manager.get_gps()
        assert result is not None
        assert len(result) == 6

    def test_get_gps_empty_before_load(self, fresh_save_manager):
        result = fresh_save_manager.get_gps()
        assert result is not None
        assert len(result) == 0
