"""Integration tests for PlayerOpsMixin using real save data."""

from uuid import UUID, uuid4

import pytest

from palworld_save_pal.dto.player import PlayerDTO
from palworld_save_pal.game.player import Player
from tests.game.conftest import PLAYER_O_UID, PLAYER_SKY_UID, _noop


@pytest.fixture
def sm_with_player_o(event_loop, fresh_save_manager):
    event_loop.run_until_complete(
        fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
    )
    return fresh_save_manager


@pytest.fixture
def sm_with_both_players(event_loop, fresh_save_manager):
    event_loop.run_until_complete(
        fresh_save_manager.load_player_on_demand(PLAYER_O_UID, ws_callback=_noop)
    )
    event_loop.run_until_complete(
        fresh_save_manager.load_player_on_demand(PLAYER_SKY_UID, ws_callback=_noop)
    )
    return fresh_save_manager


# ---------------------------------------------------------------------------
# get_players
# ---------------------------------------------------------------------------
class TestGetPlayers:
    def test_get_players_returns_loaded(self, sm_with_player_o):
        players = sm_with_player_o.get_players()
        assert isinstance(players, dict)
        assert PLAYER_O_UID in players

    def test_get_players_empty_before_load(self, fresh_save_manager):
        players = fresh_save_manager.get_players()
        assert len(players) == 0

    def test_get_players_two_loaded(self, sm_with_both_players):
        players = sm_with_both_players.get_players()
        assert len(players) == 2
        assert PLAYER_O_UID in players
        assert PLAYER_SKY_UID in players

    def test_get_players_values_are_player_instances(self, sm_with_player_o):
        players = sm_with_player_o.get_players()
        for uid, player in players.items():
            assert isinstance(uid, UUID)
            assert isinstance(player, Player)


# ---------------------------------------------------------------------------
# get_player
# ---------------------------------------------------------------------------
class TestGetPlayer:
    def test_get_player_existing(self, sm_with_player_o):
        player = sm_with_player_o.get_player(PLAYER_O_UID)
        assert player is not None
        assert isinstance(player, Player)
        assert player.uid == PLAYER_O_UID

    def test_get_player_properties(self, sm_with_player_o):
        player = sm_with_player_o.get_player(PLAYER_O_UID)
        assert player.nickname == "O"
        assert player.level == 65

    def test_get_player_not_loaded(self, sm_with_player_o):
        result = sm_with_player_o.get_player(PLAYER_SKY_UID)
        assert result is None

    def test_get_player_nonexistent_uuid(self, sm_with_player_o):
        result = sm_with_player_o.get_player(uuid4())
        assert result is None

    def test_get_player_empty_uuid(self, sm_with_player_o):
        empty = UUID("00000000-0000-0000-0000-000000000000")
        result = sm_with_player_o.get_player(empty)
        assert result is None

    def test_get_player_sky(self, sm_with_both_players):
        player = sm_with_both_players.get_player(PLAYER_SKY_UID)
        assert player is not None
        assert player.nickname == "sky"
        assert player.level == 2


# ---------------------------------------------------------------------------
# update_players (async)
# ---------------------------------------------------------------------------
class TestUpdatePlayers:
    def _make_player_dto(self, player):
        """Build a PlayerDTO from a live Player, including container data."""
        from palworld_save_pal.dto.item_container import ItemContainerDTO

        def _container_dto(container):
            if container is None:
                return None
            return ItemContainerDTO(
                id=container.id,
                type=container.type,
                slots=[],
                slot_num=container.slot_num,
            )

        return PlayerDTO(
            uid=player.uid,
            nickname=player.nickname,
            level=player.level,
            exp=player.exp,
            hp=player.hp,
            stomach=player.stomach,
            sanity=player.sanity,
            status_point_list=player.status_point_list,
            ext_status_point_list=player.ext_status_point_list,
            technologies=player.technologies,
            technology_points=player.technology_points,
            boss_technology_points=player.boss_technology_points,
            completed_missions=player.completed_missions,
            current_missions=player.current_missions,
            common_container=_container_dto(player.common_container),
            essential_container=_container_dto(player.essential_container),
            weapon_load_out_container=_container_dto(player.weapon_load_out_container),
            player_equipment_armor_container=_container_dto(player.player_equipment_armor_container),
            food_equip_container=_container_dto(player.food_equip_container),
        )

    def test_update_player_nickname(self, event_loop, sm_with_player_o):
        player = sm_with_player_o.get_player(PLAYER_O_UID)
        dto = self._make_player_dto(player)
        dto.nickname = "NewName"
        event_loop.run_until_complete(
            sm_with_player_o.update_players({PLAYER_O_UID: dto}, ws_callback=_noop)
        )
        updated = sm_with_player_o.get_player(PLAYER_O_UID)
        assert updated.nickname == "NewName"

    def test_update_player_level(self, event_loop, sm_with_player_o):
        player = sm_with_player_o.get_player(PLAYER_O_UID)
        dto = self._make_player_dto(player)
        dto.level = 99
        event_loop.run_until_complete(
            sm_with_player_o.update_players({PLAYER_O_UID: dto}, ws_callback=_noop)
        )
        updated = sm_with_player_o.get_player(PLAYER_O_UID)
        assert updated.level == 99

    def test_update_players_empty_dict(self, event_loop, sm_with_player_o):
        event_loop.run_until_complete(
            sm_with_player_o.update_players({}, ws_callback=_noop)
        )
        player = sm_with_player_o.get_player(PLAYER_O_UID)
        assert player.nickname == "O"

    def test_update_players_no_gvas_raises(self, event_loop, fresh_save_manager):
        fresh_save_manager._gvas_file = None
        dto = PlayerDTO(uid=uuid4(), nickname="X", level=1, exp=0)
        with pytest.raises(ValueError, match="No GvasFile"):
            event_loop.run_until_complete(
                fresh_save_manager.update_players({dto.uid: dto}, ws_callback=_noop)
            )


# ---------------------------------------------------------------------------
# update_player_technologies (async)
# ---------------------------------------------------------------------------
class TestUpdatePlayerTechnologies:
    def test_update_technologies_list(self, event_loop, sm_with_player_o):
        new_techs = ["SomeRecipe1", "SomeRecipe2"]
        event_loop.run_until_complete(
            sm_with_player_o.update_player_technologies(
                player_id=PLAYER_O_UID,
                technologies=new_techs,
                ws_callback=_noop,
            )
        )
        player = sm_with_player_o.get_player(PLAYER_O_UID)
        assert player.technologies == new_techs

    def test_update_technology_points(self, event_loop, sm_with_player_o):
        event_loop.run_until_complete(
            sm_with_player_o.update_player_technologies(
                player_id=PLAYER_O_UID,
                technology_points=999,
                ws_callback=_noop,
            )
        )
        player = sm_with_player_o.get_player(PLAYER_O_UID)
        assert player.technology_points == 999

    def test_update_boss_technology_points(self, event_loop, sm_with_player_o):
        event_loop.run_until_complete(
            sm_with_player_o.update_player_technologies(
                player_id=PLAYER_O_UID,
                boss_technology_points=50,
                ws_callback=_noop,
            )
        )
        player = sm_with_player_o.get_player(PLAYER_O_UID)
        assert player.boss_technology_points == 50

    def test_update_technologies_missing_player_raises(
        self, event_loop, sm_with_player_o
    ):
        with pytest.raises(ValueError, match="not found"):
            event_loop.run_until_complete(
                sm_with_player_o.update_player_technologies(
                    player_id=uuid4(),
                    technologies=["X"],
                    ws_callback=_noop,
                )
            )

    def test_update_technologies_no_gvas_raises(self, event_loop, fresh_save_manager):
        fresh_save_manager._gvas_file = None
        with pytest.raises(ValueError, match="No GvasFile"):
            event_loop.run_until_complete(
                fresh_save_manager.update_player_technologies(
                    player_id=uuid4(),
                    technologies=["X"],
                    ws_callback=_noop,
                )
            )

    def test_update_technologies_none_values_no_change(
        self, event_loop, sm_with_player_o
    ):
        player = sm_with_player_o.get_player(PLAYER_O_UID)
        original_points = player.technology_points
        original_boss_points = player.boss_technology_points
        original_techs = list(player.technologies)

        event_loop.run_until_complete(
            sm_with_player_o.update_player_technologies(
                player_id=PLAYER_O_UID,
                ws_callback=_noop,
            )
        )
        player = sm_with_player_o.get_player(PLAYER_O_UID)
        assert player.technology_points == original_points
        assert player.boss_technology_points == original_boss_points
        assert player.technologies == original_techs


# ---------------------------------------------------------------------------
# delete_player (async)
# ---------------------------------------------------------------------------
class TestDeletePlayer:
    def test_delete_guild_admin_returns_false(self, event_loop, sm_with_both_players):
        # Both players are guild admins in this save, so delete should be refused
        result = event_loop.run_until_complete(
            sm_with_both_players.delete_player(PLAYER_SKY_UID, ws_callback=_noop)
        )
        assert result is False

    def test_delete_guild_admin_keeps_player(self, event_loop, sm_with_both_players):
        event_loop.run_until_complete(
            sm_with_both_players.delete_player(PLAYER_SKY_UID, ws_callback=_noop)
        )
        # Player should still exist since delete was refused
        assert sm_with_both_players.get_player(PLAYER_SKY_UID) is not None

    def test_delete_player_missing_raises(self, event_loop, sm_with_player_o):
        with pytest.raises(ValueError, match="not found"):
            event_loop.run_until_complete(
                sm_with_player_o.delete_player(uuid4(), ws_callback=_noop)
            )
