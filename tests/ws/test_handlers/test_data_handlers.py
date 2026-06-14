"""Tests for data-retrieval handlers that read JSON files and return localized data.

Covers: active_skills, elements, work_suitability, passive_skills, items,
buildings, and missions handlers.
"""

import pytest
from unittest.mock import MagicMock, patch


class MockWebSocket:
    def __init__(self):
        self.sent = []

    async def send_json(self, data):
        self.sent.append(data)


@pytest.fixture
def mock_app_state():
    state = MagicMock()
    state.settings.language = "en"
    return state


# ---- active_skills ----


class TestGetActiveSkillsHandler:
    @pytest.mark.asyncio
    async def test_returns_localized_active_skills(self, mock_app_state):
        base_data = {
            "AirCanon": {"element": "Normal", "power": 25},
        }
        i18n_data = {
            "AirCanon": {
                "localized_name": "Air Cannon",
                "description": "Fires air.",
            },
        }

        with (
            patch(
                "palworld_save_pal.ws.handlers.active_skills_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.active_skills_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.active_skills_handler import (
                get_active_skills_handler,
            )

            ws = MockWebSocket()
            await get_active_skills_handler(MagicMock(), ws)

            assert len(ws.sent) == 1
            resp = ws.sent[0]
            assert resp["type"] == "get_active_skills"
            assert "AirCanon" in resp["data"]
            skill = resp["data"]["AirCanon"]
            assert skill["localized_name"] == "Air Cannon"
            assert skill["description"] == "Fires air."
            assert skill["id"] == "AirCanon"
            assert skill["details"]["element"] == "Normal"

    @pytest.mark.asyncio
    async def test_missing_i18n_uses_fallback(self, mock_app_state):
        base_data = {"UnknownSkill": {"power": 10}}
        i18n_data = {}

        with (
            patch(
                "palworld_save_pal.ws.handlers.active_skills_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.active_skills_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.active_skills_handler import (
                get_active_skills_handler,
            )

            ws = MockWebSocket()
            await get_active_skills_handler(MagicMock(), ws)

            skill = ws.sent[0]["data"]["UnknownSkill"]
            assert skill["localized_name"] == "UnknownSkill"
            assert skill["description"] == ""


# ---- elements ----


class TestGetElementsHandler:
    @pytest.mark.asyncio
    async def test_returns_localized_elements(self, mock_app_state):
        base_data = {"Fire": {"icon": "fire.png"}}
        i18n_data = {"Fire": {"localized_name": "Flame"}}

        with (
            patch(
                "palworld_save_pal.ws.handlers.elements_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.elements_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.elements_handler import (
                get_elements_handler,
            )

            ws = MockWebSocket()
            await get_elements_handler(MagicMock(), ws)

            resp = ws.sent[0]
            assert resp["type"] == "get_elements"
            elem = resp["data"]["Fire"]
            assert elem["localized_name"] == "Flame"
            assert elem["icon"] == "fire.png"

    @pytest.mark.asyncio
    async def test_missing_i18n_uses_id_as_name(self, mock_app_state):
        base_data = {"Water": {"icon": "water.png"}}
        i18n_data = {}

        with (
            patch(
                "palworld_save_pal.ws.handlers.elements_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.elements_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.elements_handler import (
                get_elements_handler,
            )

            ws = MockWebSocket()
            await get_elements_handler(MagicMock(), ws)

            elem = ws.sent[0]["data"]["Water"]
            assert elem["localized_name"] == "Water"


# ---- work_suitability ----


class TestGetWorkSuitabilityHandler:
    @pytest.mark.asyncio
    async def test_returns_work_suitability_data(self, mock_app_state):
        ws_data = {"Kindling": {"localized_name": "Kindling"}}

        with (
            patch(
                "palworld_save_pal.ws.handlers.work_suitability_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.work_suitability_handler.JsonManager"
            ) as MockJM,
        ):
            instance = MagicMock()
            instance.read.return_value = ws_data
            MockJM.return_value = instance

            from palworld_save_pal.ws.handlers.work_suitability_handler import (
                get_work_suitability_handler,
            )

            ws = MockWebSocket()
            await get_work_suitability_handler(MagicMock(), ws)

            resp = ws.sent[0]
            assert resp["type"] == "get_work_suitability"
            assert resp["data"] == ws_data


# ---- passive_skills ----


class TestGetPassiveSkillsHandler:
    @pytest.mark.asyncio
    async def test_returns_localized_passive_skills(self, mock_app_state):
        base_data = {"Legend": {"rank": 4}}
        i18n_data = {
            "Legend": {"localized_name": "Legend", "description": "Legendary passive."}
        }

        with (
            patch(
                "palworld_save_pal.ws.handlers.passive_skills_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.passive_skills_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.passive_skills_handler import (
                get_passive_skills_handler,
            )

            ws = MockWebSocket()
            await get_passive_skills_handler(MagicMock(), ws)

            resp = ws.sent[0]
            assert resp["type"] == "get_passive_skills"
            skill = resp["data"]["Legend"]
            assert skill["localized_name"] == "Legend"
            assert skill["description"] == "Legendary passive."
            assert skill["details"]["rank"] == 4


# ---- items ----


class TestGetItemsHandler:
    @pytest.mark.asyncio
    async def test_returns_localized_items(self, mock_app_state):
        base_data = {"PalSphere": {"weight": 1.0}}
        i18n_data = {
            "PalSphere": {
                "localized_name": "Pal Sphere",
                "description": "Catches pals.",
            }
        }

        with (
            patch(
                "palworld_save_pal.ws.handlers.items_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.items_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.items_handler import get_items_handler

            ws = MockWebSocket()
            await get_items_handler(MagicMock(), ws)

            resp = ws.sent[0]
            assert resp["type"] == "get_items"
            item = resp["data"]["PalSphere"]
            assert item["id"] == "PalSphere"
            assert item["details"]["weight"] == 1.0
            assert item["info"]["localized_name"] == "Pal Sphere"

    @pytest.mark.asyncio
    async def test_missing_i18n_uses_id(self, mock_app_state):
        base_data = {"Rare": {"weight": 2.0}}
        i18n_data = {}

        with (
            patch(
                "palworld_save_pal.ws.handlers.items_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.items_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.items_handler import get_items_handler

            ws = MockWebSocket()
            await get_items_handler(MagicMock(), ws)

            item = ws.sent[0]["data"]["Rare"]
            assert item["info"]["localized_name"] == "Rare"
            assert item["info"]["description"] == ""


# ---- buildings ----


class TestGetBuildingsHandler:
    @pytest.mark.asyncio
    async def test_returns_localized_buildings(self, mock_app_state):
        base_data = {"Campfire": {"category": "utility"}}
        i18n_data = {
            "Campfire": {
                "localized_name": "Campfire",
                "description": "A warm fire.",
            }
        }

        with (
            patch(
                "palworld_save_pal.ws.handlers.buildings_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.buildings_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.buildings_handler import (
                get_buildings_handler,
            )

            ws = MockWebSocket()
            await get_buildings_handler(MagicMock(), ws)

            resp = ws.sent[0]
            assert resp["type"] == "get_buildings"
            bldg = resp["data"]["Campfire"]
            assert bldg["localized_name"] == "Campfire"
            assert bldg["description"] == "A warm fire."
            assert bldg["category"] == "utility"


# ---- missions ----


class TestGetMissionsHandler:
    @pytest.mark.asyncio
    async def test_returns_localized_missions(self, mock_app_state):
        base_data = {
            "Mission1": {
                "quest_type": "Main",
                "rewards": {"gold": 100},
            }
        }
        i18n_data = {
            "Mission1": {
                "localized_name": "First Mission",
                "description": "Your first quest.",
            }
        }

        with (
            patch(
                "palworld_save_pal.ws.handlers.missions_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.missions_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.missions_handler import (
                get_missions_handler,
            )

            ws = MockWebSocket()
            await get_missions_handler(MagicMock(), ws)

            resp = ws.sent[0]
            assert resp["type"] == "get_missions"
            mission = resp["data"]["Mission1"]
            assert mission["id"] == "Mission1"
            assert mission["localized_name"] == "First Mission"
            assert mission["description"] == "Your first quest."
            assert mission["quest_type"] == "Main"
            assert mission["rewards"] == {"gold": 100}

    @pytest.mark.asyncio
    async def test_missing_i18n_defaults(self, mock_app_state):
        base_data = {"Mission2": {"quest_type": "Side"}}
        i18n_data = {}

        with (
            patch(
                "palworld_save_pal.ws.handlers.missions_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.missions_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.missions_handler import (
                get_missions_handler,
            )

            ws = MockWebSocket()
            await get_missions_handler(MagicMock(), ws)

            mission = ws.sent[0]["data"]["Mission2"]
            assert mission["localized_name"] == "Mission2"
            assert mission["description"] == ""

    @pytest.mark.asyncio
    async def test_missing_quest_type_defaults_to_main(self, mock_app_state):
        base_data = {"Mission3": {}}
        i18n_data = {}

        with (
            patch(
                "palworld_save_pal.ws.handlers.missions_handler.get_app_state",
                return_value=mock_app_state,
            ),
            patch(
                "palworld_save_pal.ws.handlers.missions_handler.JsonManager"
            ) as MockJM,
        ):
            instances = [MagicMock(), MagicMock()]
            instances[0].read.return_value = base_data
            instances[1].read.return_value = i18n_data
            MockJM.side_effect = instances

            from palworld_save_pal.ws.handlers.missions_handler import (
                get_missions_handler,
            )

            ws = MockWebSocket()
            await get_missions_handler(MagicMock(), ws)

            mission = ws.sent[0]["data"]["Mission3"]
            assert mission["quest_type"] == "Main"
            assert mission["rewards"] == {}
