use ps_server::bridge::protocol::{BridgeEndpointFile, BridgeEnvelope, BridgeErrorData};

const AUTH: &str = include_str!("../../ps-amity/fixtures/auth.json");
const AUTH_OK: &str = include_str!("../../ps-amity/fixtures/auth_ok.json");
const CAPABILITIES: &str = include_str!("../../ps-amity/fixtures/capabilities.json");
const COMMAND_HEAL: &str = include_str!("../../ps-amity/fixtures/command_heal.json");
const COMMAND_SET_ITEM_SLOT: &str =
    include_str!("../../ps-amity/fixtures/command_set_item_slot.json");
const COMMAND_RESULT_HEAL: &str = include_str!("../../ps-amity/fixtures/command_result_heal.json");
const COMMAND_RESULT_SET_ITEM_SLOT: &str =
    include_str!("../../ps-amity/fixtures/command_result_set_item_slot.json");
const ERROR_CAPABILITY_UNAVAILABLE: &str =
    include_str!("../../ps-amity/fixtures/error_capability_unavailable.json");
const ERROR_GAME_ERROR: &str = include_str!("../../ps-amity/fixtures/error_game_error.json");
const ERROR_NOT_AUTHORITATIVE: &str =
    include_str!("../../ps-amity/fixtures/error_not_authoritative.json");
const ERROR_QUEUE_FULL: &str = include_str!("../../ps-amity/fixtures/error_queue_full.json");
const ERROR_SHUTTING_DOWN: &str = include_str!("../../ps-amity/fixtures/error_shutting_down.json");
const ERROR_TIMEOUT: &str = include_str!("../../ps-amity/fixtures/error_timeout.json");
const ERROR_UNAUTHORIZED: &str = include_str!("../../ps-amity/fixtures/error_unauthorized.json");
const ERROR_VALIDATION_FAILED: &str =
    include_str!("../../ps-amity/fixtures/error_validation_failed.json");
const GET_CAPABILITIES: &str = include_str!("../../ps-amity/fixtures/get_capabilities.json");
const GET_INVENTORY: &str = include_str!("../../ps-amity/fixtures/get_inventory.json");
const GET_PAL_DETAIL: &str = include_str!("../../ps-amity/fixtures/get_pal_detail.json");
const GET_PALS: &str = include_str!("../../ps-amity/fixtures/get_pals.json");
const GET_PLAYERS: &str = include_str!("../../ps-amity/fixtures/get_players.json");
const GET_STATUS: &str = include_str!("../../ps-amity/fixtures/get_status.json");
const HELLO: &str = include_str!("../../ps-amity/fixtures/hello.json");
const HELLO_OK: &str = include_str!("../../ps-amity/fixtures/hello_ok.json");
const INVENTORY: &str = include_str!("../../ps-amity/fixtures/inventory.json");
const PAL_DETAIL: &str = include_str!("../../ps-amity/fixtures/pal_detail.json");
const PALS: &str = include_str!("../../ps-amity/fixtures/pals.json");
const PLAYERS: &str = include_str!("../../ps-amity/fixtures/players.json");
const STATUS: &str = include_str!("../../ps-amity/fixtures/status.json");

fn envelope_of(json: &str, expected_kind: &str) -> BridgeEnvelope {
    let envelope: BridgeEnvelope =
        serde_json::from_str(json).expect("fixture should parse as BridgeEnvelope");
    assert_eq!(envelope.kind, expected_kind);
    envelope
}

#[test]
fn all_fixtures_parse_as_bridge_envelope_with_expected_kind() {
    envelope_of(AUTH, "auth");
    envelope_of(AUTH_OK, "auth_ok");
    envelope_of(CAPABILITIES, "capabilities");
    envelope_of(COMMAND_HEAL, "command");
    envelope_of(COMMAND_SET_ITEM_SLOT, "command");
    envelope_of(COMMAND_RESULT_HEAL, "command_result");
    envelope_of(COMMAND_RESULT_SET_ITEM_SLOT, "command_result");
    envelope_of(ERROR_CAPABILITY_UNAVAILABLE, "error");
    envelope_of(ERROR_GAME_ERROR, "error");
    envelope_of(ERROR_NOT_AUTHORITATIVE, "error");
    envelope_of(ERROR_QUEUE_FULL, "error");
    envelope_of(ERROR_SHUTTING_DOWN, "error");
    envelope_of(ERROR_TIMEOUT, "error");
    envelope_of(ERROR_UNAUTHORIZED, "error");
    envelope_of(ERROR_VALIDATION_FAILED, "error");
    envelope_of(GET_CAPABILITIES, "get_capabilities");
    envelope_of(GET_INVENTORY, "get_inventory");
    envelope_of(GET_PAL_DETAIL, "get_pal_detail");
    envelope_of(GET_PALS, "get_pals");
    envelope_of(GET_PLAYERS, "get_players");
    envelope_of(GET_STATUS, "get_status");
    envelope_of(HELLO, "hello");
    envelope_of(HELLO_OK, "hello_ok");
    envelope_of(INVENTORY, "inventory");
    envelope_of(PAL_DETAIL, "pal_detail");
    envelope_of(PALS, "pals");
    envelope_of(PLAYERS, "players");
    envelope_of(STATUS, "status");
}

#[test]
fn status_fixture_carries_the_documented_fields() {
    let envelope = envelope_of(STATUS, "status");
    assert_eq!(envelope.data["mode"], "coop_host");
    assert_eq!(envelope.data["authoritative"], true);
    assert_eq!(envelope.data["protocolVersion"], 2);
    assert_eq!(envelope.data["queueDepth"], 0);
    assert_eq!(envelope.data["worldLoaded"], true);
    assert_eq!(envelope.data["modVersion"], "0.2.0");
}

#[test]
fn players_fixture_carries_one_fully_populated_player() {
    let envelope = envelope_of(PLAYERS, "players");
    assert_eq!(envelope.data["status"], "ok");
    let players = envelope.data["players"]
        .as_array()
        .expect("players should be an array");
    assert_eq!(players.len(), 1);
    let player = &players[0];
    assert_eq!(player["uid"], "00000000-0000-0000-0000-000000000001");
    assert_eq!(player["level"], 80);
    assert_eq!(player["exp"], 2354280);
    assert_eq!(player["yaw"], 26.3);
    assert_eq!(player["status"], "ok");
}

#[test]
fn error_timeout_fixture_parses_to_typed_error_data() {
    let envelope = envelope_of(ERROR_TIMEOUT, "error");
    let payload: BridgeErrorData =
        serde_json::from_value(envelope.data).expect("error data should parse");
    assert_eq!(payload.code, "timeout");
}

#[test]
fn command_result_fixtures_carry_the_result_envelope_fields() {
    let envelope = envelope_of(COMMAND_RESULT_HEAL, "command_result");
    assert_eq!(envelope.data["commandId"], "heal-cmd-1");
    assert_eq!(envelope.data["op"], "pal.heal");
    assert_eq!(envelope.data["applied"], true);
    assert_eq!(envelope.data["verified"], true);
    assert_eq!(envelope.data["retrySafe"], true);
    assert_eq!(envelope.data["data"]["steps"]["hp"], "ok");

    let envelope = envelope_of(COMMAND_RESULT_SET_ITEM_SLOT, "command_result");
    assert_eq!(envelope.data["commandId"], "set-slot-cmd-1");
    assert_eq!(envelope.data["op"], "item.setSlot");
    assert_eq!(envelope.data["applied"], true);
    assert_eq!(envelope.data["verified"], true);
    assert_eq!(envelope.data["retrySafe"], false);
    assert_eq!(envelope.data["data"]["slotBefore"]["count"], 0);
    assert_eq!(envelope.data["data"]["slotAfter"]["staticItemId"], "Wood");
    assert_eq!(envelope.data["data"]["slotAfter"]["count"], 20);
}

#[test]
fn command_request_fixtures_carry_the_expected_op_and_command_id() {
    for (fixture, expected_command_id, expected_op) in [
        (COMMAND_HEAL, "heal-cmd-1", "pal.heal"),
        (COMMAND_SET_ITEM_SLOT, "set-slot-cmd-1", "item.setSlot"),
    ] {
        let envelope = envelope_of(fixture, "command");
        assert_eq!(envelope.data["commandId"], expected_command_id);
        assert_eq!(envelope.data["op"], expected_op);
    }
}

#[test]
fn command_result_fixtures_echo_their_request_ids() {
    for (request, result) in [
        (COMMAND_HEAL, COMMAND_RESULT_HEAL),
        (COMMAND_SET_ITEM_SLOT, COMMAND_RESULT_SET_ITEM_SLOT),
    ] {
        let request = envelope_of(request, "command");
        let result = envelope_of(result, "command_result");
        assert_eq!(result.id, request.id);
        assert_eq!(result.data["commandId"], request.data["commandId"]);
        assert_eq!(result.data["op"], request.data["op"]);
    }
}

#[test]
fn capabilities_fixture_reports_availability_per_op() {
    let envelope = envelope_of(CAPABILITIES, "capabilities");
    assert_eq!(envelope.data["version"], 1);
    assert_eq!(envelope.data["ops"]["pal.heal"]["available"], true);
    assert_eq!(envelope.data["ops"]["pal.heal"]["reason"], serde_json::Value::Null);
    assert_eq!(envelope.data["ops"]["player.edit"]["available"], true);
    assert_eq!(envelope.data["ops"]["item.setSlot"]["available"], false);
    assert_eq!(
        envelope.data["ops"]["item.setSlot"]["reason"],
        "item database not loaded"
    );
}

#[test]
fn new_error_fixtures_parse_to_typed_error_data() {
    for (fixture, expected_code) in [
        (ERROR_CAPABILITY_UNAVAILABLE, "capability_unavailable"),
        (ERROR_NOT_AUTHORITATIVE, "not_authoritative"),
        (ERROR_QUEUE_FULL, "queue_full"),
        (ERROR_VALIDATION_FAILED, "validation_failed"),
        (ERROR_GAME_ERROR, "game_error"),
        (ERROR_SHUTTING_DOWN, "shutting_down"),
        (ERROR_UNAUTHORIZED, "unauthorized"),
    ] {
        let envelope = envelope_of(fixture, "error");
        let payload: BridgeErrorData =
            serde_json::from_value(envelope.data).expect("error data should parse");
        assert_eq!(payload.code, expected_code);
    }
}

#[test]
fn endpoint_file_parses_from_literal_json() {
    let json = r#"{"protocolVersion":1,"port":38217,"token":"abc123","pid":4242,"startedAt":"2026-09-03T00:00:00Z"}"#;
    let endpoint: BridgeEndpointFile =
        serde_json::from_str(json).expect("endpoint file should parse");
    assert_eq!(endpoint.protocol_version, 1);
    assert_eq!(endpoint.port, 38217);
    assert_eq!(endpoint.token, "abc123");
    assert_eq!(endpoint.pid, 4242);
    assert_eq!(endpoint.started_at, "2026-09-03T00:00:00Z");
}

#[test]
fn envelope_with_unknown_extra_data_keys_still_parses() {
    let json = r#"{"id":"99","type":"status","data":{"authoritative":true,"modVersion":"0.1.0","mode":"coop_host","protocolVersion":1,"queueDepth":0,"worldLoaded":true,"futureField":"ignored"}}"#;
    let envelope: BridgeEnvelope =
        serde_json::from_str(json).expect("envelope with unknown extra keys should still parse");
    assert_eq!(envelope.kind, "status");
    assert_eq!(envelope.data["mode"], "coop_host");
    assert_eq!(envelope.data["futureField"], "ignored");
}

#[test]
fn bridge_envelope_round_trips_through_serialize_deserialize() {
    let original = envelope_of(STATUS, "status");
    let serialized = serde_json::to_string(&original).expect("envelope should serialize");
    let reparsed: BridgeEnvelope =
        serde_json::from_str(&serialized).expect("serialized envelope should reparse");
    assert_eq!(original, reparsed);
}
