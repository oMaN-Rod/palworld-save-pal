//! Smoke test: the Palworld 1.0 fixtures parse with the current reader.

mod common;

#[test]
fn v1_relics_fixture_loads() {
    let session = common::load_fixture_session("v1_relics");
    assert!(
        !session.player_file_refs.is_empty(),
        "v1_relics must carry player saves"
    );
}

#[test]
fn v1_stats_fixture_loads() {
    let session = common::load_fixture_session("v1_stats");
    assert!(
        !session.player_file_refs.is_empty(),
        "v1_stats must carry player saves"
    );
}
