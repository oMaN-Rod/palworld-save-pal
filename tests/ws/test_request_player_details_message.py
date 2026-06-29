from uuid import UUID

from palworld_save_pal.ws.messages import RequestPlayerDetailsMessage


def test_request_player_details_parses_player_id_and_origin():
    msg = RequestPlayerDetailsMessage(
        data={"player_id": "8c2f1930-0000-0000-0000-000000000000", "origin": "bulk"}
    )
    assert msg.data.player_id == UUID("8c2f1930-0000-0000-0000-000000000000")
    assert msg.data.origin == "bulk"


def test_request_player_details_origin_defaults_to_edit():
    msg = RequestPlayerDetailsMessage(
        data={"player_id": "8c2f1930-0000-0000-0000-000000000000"}
    )
    assert msg.data.origin == "edit"
