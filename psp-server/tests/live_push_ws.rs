mod common;

use psp_app::live::{LiveFrame, LiveSourceKind};

fn test_frame(seq: u64) -> LiveFrame {
    LiveFrame {
        seq,
        source: LiveSourceKind::File,
        captured_at_ms: 0,
        observed_at_ms: 0,
        fps: None,
        ingame_time: None,
        ingame_days: None,
        actors: vec![],
    }
}

#[tokio::test]
async fn subscribe_live_pushes_bus_frames() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    common::send_json(&mut ws, serde_json::json!({ "type": "subscribe_live" })).await;
    let ack = common::next_json(&mut ws).await;
    assert_eq!(ack["type"], "subscribe_live");

    server
        .handle
        .app
        .live_bus
        .send(Some(test_frame(1)))
        .unwrap();
    let push = common::next_json(&mut ws).await;
    assert_eq!(push["type"], "live_frame");
    assert_eq!(push["data"]["seq"], 1);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn late_subscriber_receives_the_frame_already_on_the_bus() {
    let server = common::start_test_server().await;
    server
        .handle
        .app
        .live_bus
        .send(Some(test_frame(7)))
        .unwrap();

    let mut ws = common::connect(&server).await;
    common::send_json(&mut ws, serde_json::json!({ "type": "subscribe_live" })).await;
    let ack = common::next_json(&mut ws).await;
    assert_eq!(ack["type"], "subscribe_live");

    let push = common::next_json(&mut ws).await;
    assert_eq!(push["type"], "live_frame");
    assert_eq!(push["data"]["seq"], 7);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_second_subscribe_live_acks_but_does_not_double_push() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(&mut ws, serde_json::json!({ "type": "subscribe_live" })).await;
    let ack = common::next_json(&mut ws).await;
    assert_eq!(ack["type"], "subscribe_live");

    common::send_json(&mut ws, serde_json::json!({ "type": "subscribe_live" })).await;
    let second_ack = common::next_json(&mut ws).await;
    assert_eq!(second_ack["type"], "subscribe_live");

    server
        .handle
        .app
        .live_bus
        .send(Some(test_frame(1)))
        .unwrap();
    let push = common::next_json(&mut ws).await;
    assert_eq!(push["type"], "live_frame");
    assert_eq!(push["data"]["seq"], 1);

    server
        .handle
        .app
        .live_bus
        .send(Some(test_frame(2)))
        .unwrap();
    let next_push = common::next_json(&mut ws).await;
    assert_eq!(next_push["type"], "live_frame");
    assert_eq!(next_push["data"]["seq"], 2);

    server.handle.shutdown().await;
}
