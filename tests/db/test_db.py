"""
Database tests using in-memory SQLite.

We monkeypatch the module-level `engine` used by ctx functions so all
operations go through a fresh in-memory database per test.
"""

from contextlib import contextmanager
from datetime import datetime
from uuid import UUID, uuid4

import pytest
from sqlalchemy import create_engine
from sqlmodel import Session, SQLModel

from palworld_save_pal.db.models.settings_model import SettingsModel
from palworld_save_pal.db.models.server_models import ServerModel
from palworld_save_pal.db.models.ups_models import (
    UPSCollectionModel,
    UPSPalModel,
    UPSStatsModel,
    UPSTagModel,
    UPSTransferLogModel,
)
from palworld_save_pal.dto.settings import SettingsDTO
from palworld_save_pal.editor.preset_profile import PalPreset, PresetProfile

# ---------------------------------------------------------------------------
# Shared fixture: in-memory engine that replaces the real one
# ---------------------------------------------------------------------------

_test_engine = None


@pytest.fixture(autouse=True)
def _patch_db(monkeypatch):
    """Replace the global engine in every module that imports it."""
    global _test_engine
    _test_engine = create_engine("sqlite:///:memory:")
    SQLModel.metadata.create_all(_test_engine)

    # Patch the engine in bootstrap (source of truth)
    import palworld_save_pal.db.bootstrap as bootstrap_mod

    monkeypatch.setattr(bootstrap_mod, "engine", _test_engine)

    # Patch in ctx modules that import engine directly
    import palworld_save_pal.db.ctx.ups as ups_mod
    import palworld_save_pal.db.ctx.servers as servers_mod

    monkeypatch.setattr(ups_mod, "engine", _test_engine)
    monkeypatch.setattr(servers_mod, "engine", _test_engine)

    # Patch get_db_session in ctx.utils to use our engine
    import palworld_save_pal.db.ctx.utils as utils_mod

    @contextmanager
    def _test_get_db_session():
        session = Session(_test_engine)
        try:
            yield session
            session.commit()
        except Exception as e:
            session.rollback()
            raise e
        finally:
            session.close()

    monkeypatch.setattr(utils_mod, "get_db_session", _test_get_db_session)

    # Also patch in modules that import get_db_session
    import palworld_save_pal.db.ctx.settings as settings_mod
    import palworld_save_pal.db.ctx.presets as presets_mod

    monkeypatch.setattr(settings_mod, "get_db_session", _test_get_db_session)
    monkeypatch.setattr(presets_mod, "get_db_session", _test_get_db_session)

    yield


# ===========================================================================
# Settings tests
# ===========================================================================


class TestSettingsCtx:
    def test_get_settings_creates_default(self):
        from palworld_save_pal.db.ctx.settings import get_settings

        settings = get_settings()
        assert isinstance(settings, SettingsModel)
        assert settings.id == 1
        assert settings.language == "en"

    def test_get_settings_idempotent(self):
        from palworld_save_pal.db.ctx.settings import get_settings

        s1 = get_settings()
        s2 = get_settings()
        assert s1.id == s2.id

    def test_update_settings(self):
        from palworld_save_pal.db.ctx.settings import get_settings, update_settings

        get_settings()  # ensure exists
        dto = SettingsDTO(
            language="ja",
            clone_prefix="X",
            new_pal_prefix="Y",
            debug_mode=True,
            cheat_mode=False,
        )
        update_settings(dto)
        # Re-fetch to verify (update_settings doesn't expunge)
        result = get_settings()
        assert result.language == "ja"
        assert result.clone_prefix == "X"

    def test_update_save_dir(self):
        from palworld_save_pal.db.ctx.settings import get_settings, update_save_dir

        get_settings()
        update_save_dir("/new/path")
        # Re-fetch to verify
        result = get_settings()
        assert result.save_dir == "/new/path"


# ===========================================================================
# Server tests
# ===========================================================================


class TestServerDBService:
    def _server_data(self, **overrides):
        data = {
            "name": "Test Server",
            "container_name": f"test-server-{uuid4().hex[:8]}",
            "data_volume_name": "vol1",
            "saves_path": "/saves",
            "mods_path": "/mods",
            "logicmods_path": "/logicmods",
            "nativemods_path": "/nativemods",
        }
        data.update(overrides)
        return data

    def test_create_server(self):
        from palworld_save_pal.db.ctx.servers import ServerDBService

        server = ServerDBService.create_server(self._server_data(name="MyServer"))
        assert server.id is not None
        assert server.name == "MyServer"
        assert server.game_port == 8211  # default

    def test_get_server(self):
        from palworld_save_pal.db.ctx.servers import ServerDBService

        created = ServerDBService.create_server(self._server_data())
        fetched = ServerDBService.get_server(created.id)
        assert fetched is not None
        assert fetched.name == created.name

    def test_get_server_not_found(self):
        from palworld_save_pal.db.ctx.servers import ServerDBService

        assert ServerDBService.get_server(9999) is None

    def test_get_server_by_container_name(self):
        from palworld_save_pal.db.ctx.servers import ServerDBService

        data = self._server_data(container_name="unique-name")
        ServerDBService.create_server(data)
        fetched = ServerDBService.get_server_by_container_name("unique-name")
        assert fetched is not None
        assert fetched.container_name == "unique-name"

    def test_list_servers(self):
        from palworld_save_pal.db.ctx.servers import ServerDBService

        ServerDBService.create_server(self._server_data())
        ServerDBService.create_server(self._server_data())
        servers = ServerDBService.list_servers()
        assert len(servers) == 2

    def test_update_server(self):
        from palworld_save_pal.db.ctx.servers import ServerDBService

        created = ServerDBService.create_server(self._server_data())
        updated = ServerDBService.update_server(created.id, {"name": "Updated"})
        assert updated.name == "Updated"

    def test_update_server_not_found(self):
        from palworld_save_pal.db.ctx.servers import ServerDBService

        assert ServerDBService.update_server(9999, {"name": "x"}) is None

    def test_delete_server(self):
        from palworld_save_pal.db.ctx.servers import ServerDBService

        created = ServerDBService.create_server(self._server_data())
        assert ServerDBService.delete_server(created.id) is True
        assert ServerDBService.get_server(created.id) is None

    def test_delete_server_not_found(self):
        from palworld_save_pal.db.ctx.servers import ServerDBService

        assert ServerDBService.delete_server(9999) is False

    def test_get_allocated_ports(self):
        from palworld_save_pal.db.ctx.servers import ServerDBService

        ServerDBService.create_server(
            self._server_data(game_port=8211, query_port=27015, rest_api_port=8212)
        )
        ports = ServerDBService.get_allocated_ports()
        assert 8211 in ports
        assert 27015 in ports
        assert 8212 in ports


# ===========================================================================
# Presets tests
# ===========================================================================


class TestPresetsCtx:
    def test_add_and_get_preset(self):
        from palworld_save_pal.db.ctx.presets import add_preset, get_all_presets

        preset_id = add_preset({"name": "TestPreset", "type": "pal"})
        assert preset_id is not None

        presets = get_all_presets()
        assert preset_id in presets
        assert presets[preset_id]["name"] == "TestPreset"

    def test_add_preset_with_pal_preset(self):
        from palworld_save_pal.db.ctx.presets import add_preset, get_all_presets

        preset_id = add_preset({
            "name": "WithPal",
            "type": "pal",
            "pal_preset": {"lock": True, "level": 50},
        })
        presets = get_all_presets()
        assert "pal_preset" in presets[preset_id]
        assert presets[preset_id]["pal_preset"]["level"] == 50

    def test_delete_preset(self):
        from palworld_save_pal.db.ctx.presets import (
            add_preset,
            delete_preset,
            get_all_presets,
        )

        preset_id = add_preset({"name": "ToDelete", "type": "pal"})
        assert delete_preset(preset_id) is True
        presets = get_all_presets()
        assert preset_id not in presets

    def test_delete_nonexistent_preset(self):
        from palworld_save_pal.db.ctx.presets import delete_preset

        assert delete_preset("nonexistent-id") is False

    def test_nuke_presets(self):
        from palworld_save_pal.db.ctx.presets import (
            add_preset,
            get_all_presets,
            nuke_presets,
        )

        add_preset({"name": "P1", "type": "pal"})
        add_preset({"name": "P2", "type": "player"})
        assert nuke_presets() is True
        assert len(get_all_presets()) == 0

    def test_update_preset_name(self):
        from palworld_save_pal.db.ctx.presets import (
            add_preset,
            get_all_presets,
            update_preset_name,
        )

        preset_id = add_preset({"name": "OldName", "type": "pal"})
        assert update_preset_name(preset_id, "NewName") is True
        presets = get_all_presets()
        assert presets[preset_id]["name"] == "NewName"


class TestAddPresetHandler:
    """Regression tests for the add_preset websocket handler."""

    class _MockWebSocket:
        def __init__(self):
            self.sent = []

        async def send_json(self, data):
            self.sent.append(data)

    @pytest.mark.asyncio
    async def test_storage_preset_with_dynamic_item_persists(self):
        """A storage preset containing a dynamic item (UUID local_id) must
        serialize into the JSON column without raising. Regression for
        'Object of type UUID is not JSON serializable'."""
        from palworld_save_pal.ws.handlers.preset_handler import add_preset_handler
        from palworld_save_pal.ws.messages import AddPresetMessage
        from palworld_save_pal.db.ctx.presets import get_all_presets

        # data is coerced into a PresetProfileDTO, which parses the local_id
        # string into a uuid.UUID object — exactly what the real handler sees.
        message = AddPresetMessage(
            data={
                "name": "StorageWithEgg",
                "type": "storage",
                "storage_container": {
                    "key": "ItemChest_04",
                    "slots": [
                        {
                            "slot_index": 4,
                            "count": 1,
                            "static_id": "PalEgg_Earth_05",
                            "dynamic_item": {
                                "local_id": "00000000-0000-0000-0000-000000000000",
                                "type": "egg",
                            },
                        }
                    ],
                },
            }
        )
        ws = self._MockWebSocket()

        await add_preset_handler(message, ws)

        assert len(ws.sent) == 1
        preset_id = ws.sent[0]["data"]["id"]

        presets = get_all_presets()
        assert preset_id in presets
        stored_local_id = presets[preset_id]["storage_container"]["slots"][0][
            "dynamic_item"
        ]["local_id"]
        assert stored_local_id == "00000000-0000-0000-0000-000000000000"
        assert isinstance(stored_local_id, str)


# ===========================================================================
# UPS Model creation tests (direct model, not full service)
# ===========================================================================


class TestUPSModels:
    def test_create_pal_model(self):
        with Session(_test_engine) as session:
            pal = UPSPalModel(
                character_id="Lambball",
                nickname="Fluffy",
                level=10,
                pal_data={"test": True},
                tags=["tag1", "tag2"],
            )
            session.add(pal)
            session.commit()
            session.refresh(pal)
            assert pal.id is not None
            assert pal.character_id == "Lambball"
            assert pal.tags == ["tag1", "tag2"]

    def test_create_collection_model(self):
        with Session(_test_engine) as session:
            collection = UPSCollectionModel(
                name="Favorites",
                description="My favorites",
                color="#ff0000",
            )
            session.add(collection)
            session.commit()
            session.refresh(collection)
            assert collection.id is not None
            assert collection.name == "Favorites"

    def test_create_tag_model(self):
        with Session(_test_engine) as session:
            tag = UPSTagModel(name="rare", color="#gold")
            session.add(tag)
            session.commit()
            session.refresh(tag)
            assert tag.id is not None
            assert tag.name == "rare"

    def test_create_stats_model(self):
        with Session(_test_engine) as session:
            stats = UPSStatsModel(id=1, total_pals=5)
            session.add(stats)
            session.commit()
            session.refresh(stats)
            assert stats.total_pals == 5

    def test_create_transfer_log(self):
        with Session(_test_engine) as session:
            pal = UPSPalModel(
                character_id="Lambball",
                level=1,
                pal_data={},
            )
            session.add(pal)
            session.commit()
            session.refresh(pal)

            log = UPSTransferLogModel(
                pal_id=pal.id,
                operation_type="import",
                source_type="pal_box",
                destination_type="ups",
                success=True,
            )
            session.add(log)
            session.commit()
            session.refresh(log)
            assert log.id is not None
            assert log.operation_type == "import"

    def test_pal_collection_relationship(self):
        with Session(_test_engine) as session:
            collection = UPSCollectionModel(name="TestCol")
            session.add(collection)
            session.commit()
            session.refresh(collection)

            pal = UPSPalModel(
                character_id="Cattiva",
                level=5,
                pal_data={},
                collection_id=collection.id,
            )
            session.add(pal)
            session.commit()
            session.refresh(pal)
            assert pal.collection_id == collection.id


# ===========================================================================
# UPS Service tests
# ===========================================================================


class TestUPSService:
    def _make_pal_dto(self, **overrides):
        from palworld_save_pal.game.enum import PalGender, WorkSuitability

        defaults = {
            "instance_id": uuid4(),
            "owner_uid": uuid4(),
            "character_id": "Lambball",
            "is_lucky": False,
            "is_boss": False,
            "gender": PalGender.FEMALE,
            "rank_hp": 0,
            "rank_attack": 0,
            "rank_defense": 0,
            "rank_craftspeed": 0,
            "talent_hp": 50,
            "talent_shot": 50,
            "talent_defense": 50,
            "rank": 1,
            "level": 10,
            "exp": 0,
            "nickname": "TestPal",
            "is_tower": False,
            "storage_id": uuid4(),
            "stomach": 300.0,
            "storage_slot": 0,
            "learned_skills": [],
            "active_skills": [],
            "passive_skills": [],
            "hp": 5000,
            "max_hp": 5000,
            "group_id": uuid4(),
            "sanity": 100.0,
            "work_suitability": {},
            "is_sick": False,
            "friendship_point": 0,
        }
        defaults.update(overrides)
        from palworld_save_pal.dto.pal import PalDTO

        return PalDTO(**defaults)

    def test_add_pal(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        dto = self._make_pal_dto()
        pal = UPSService.add_pal(dto)
        assert pal.id is not None
        assert pal.character_id == "Lambball"

    def test_get_pal_by_id(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        dto = self._make_pal_dto()
        created = UPSService.add_pal(dto)
        fetched = UPSService.get_pal_by_id(created.id)
        assert fetched is not None
        assert fetched.character_id == "Lambball"

    def test_get_pal_by_id_not_found(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        assert UPSService.get_pal_by_id(9999) is None

    def test_delete_pals(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        p1 = UPSService.add_pal(self._make_pal_dto())
        p2 = UPSService.add_pal(self._make_pal_dto(character_id="Cattiva"))
        deleted = UPSService.delete_pals([p1.id, p2.id])
        assert deleted == 2

    def test_get_pals_pagination(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        for i in range(5):
            UPSService.add_pal(self._make_pal_dto(nickname=f"Pal{i}"))

        pals, total = UPSService.get_pals(offset=0, limit=3)
        assert len(pals) == 3
        assert total == 5

    def test_create_collection(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        col = UPSService.create_collection("Favorites", "My favs", "#ff0000")
        assert col.id is not None
        assert col.name == "Favorites"

    def test_get_collections(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        UPSService.create_collection("Col1")
        UPSService.create_collection("Col2")
        cols = UPSService.get_collections()
        assert len(cols) == 2

    def test_delete_collection(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        col = UPSService.create_collection("ToDelete")
        assert UPSService.delete_collection(col.id) is True
        cols = UPSService.get_collections()
        assert len(cols) == 0

    def test_create_and_get_tags(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        tag = UPSService.create_or_update_tag("rare", color="#gold")
        assert tag.name == "rare"
        tags = UPSService.get_available_tags()
        assert len(tags) >= 1

    def test_get_stats(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        UPSService.add_pal(self._make_pal_dto())
        stats = UPSService.get_stats()
        assert stats.total_pals >= 1

    def test_nuke_all_pals(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        UPSService.add_pal(self._make_pal_dto())
        UPSService.add_pal(self._make_pal_dto())
        deleted = UPSService.nuke_all_pals()
        assert deleted >= 2
        _, total = UPSService.get_pals()
        assert total == 0

    def test_clone_pal(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        original = UPSService.add_pal(self._make_pal_dto(nickname="Original"))
        cloned = UPSService.clone_pal(original.id)
        assert cloned is not None
        assert cloned.id != original.id
        assert cloned.instance_id != original.instance_id

    def test_update_pal(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        pal = UPSService.add_pal(self._make_pal_dto(nickname="Before"))
        updated = UPSService.update_pal(pal.id, {"nickname": "After"})
        assert updated is not None
        assert updated.nickname == "After"

    def test_filter_by_search_query(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        UPSService.add_pal(self._make_pal_dto(nickname="Alpha"))
        UPSService.add_pal(self._make_pal_dto(nickname="Beta"))

        pals, total = UPSService.get_pals(search_query="alpha")
        assert total == 1
        assert pals[0].nickname == "Alpha"

    def test_filter_by_character_id(self):
        from palworld_save_pal.db.ctx.ups import UPSService

        UPSService.add_pal(self._make_pal_dto(character_id="Lambball"))
        UPSService.add_pal(self._make_pal_dto(character_id="Cattiva"))

        pals, total = UPSService.get_pals(character_id_filter="Cattiva")
        assert total == 1
