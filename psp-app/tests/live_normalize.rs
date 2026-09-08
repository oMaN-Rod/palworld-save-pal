use psp_app::live::*;

const FIXTURE: &str = include_str!("fixtures/live/world_snapshot.json");

#[test]
fn normalizes_kinds_ids_and_owner_links() {
    let f = normalize_world_json(FIXTURE, LiveSourceKind::File, 1, 42, 99).unwrap();
    assert_eq!(f.seq, 1);
    assert_eq!(f.captured_at_ms, 42);
    assert_eq!(f.observed_at_ms, 99);
    let kinds: Vec<&str> = f.actors.iter().map(|a| a.kind.as_str()).collect();
    assert!(kinds.contains(&"player"));
    assert!(kinds.contains(&"otomo"));
    assert!(kinds.contains(&"basepal"));
    assert!(kinds.contains(&"palbox"));
    assert!(kinds.contains(&"futurething"));
    let otomo = f.actors.iter().find(|a| a.kind == "otomo").unwrap();
    let player = f.actors.iter().find(|a| a.kind == "player").unwrap();
    assert_eq!(otomo.owner.as_deref(), Some(player.id.as_str()));
    assert_eq!(player.active, Some(true));
}

#[test]
fn strips_pii_from_the_wire() {
    let f = normalize_world_json(FIXTURE, LiveSourceKind::File, 1, 0, 0).unwrap();
    let s = serde_json::to_string(&f).unwrap();
    assert!(!s.contains("userid"));
    assert!(!s.contains("steam_"));
    assert!(!s.to_lowercase().contains("\"ip\""));
}

#[test]
fn palbox_without_instance_id_gets_stable_synthesized_id() {
    let f = normalize_world_json(FIXTURE, LiveSourceKind::File, 1, 0, 0).unwrap();
    let boxes: Vec<_> = f.actors.iter().filter(|a| a.kind == "palbox").collect();
    assert!(boxes.iter().all(|b| !b.id.is_empty()));
    let g = normalize_world_json(FIXTURE, LiveSourceKind::File, 2, 1, 1).unwrap();
    let boxes2: Vec<_> = g.actors.iter().filter(|a| a.kind == "palbox").collect();
    assert_eq!(boxes[0].id, boxes2[0].id);
}

#[test]
fn tolerates_bom_and_rejects_torn_json() {
    let bom = format!("\u{feff}{FIXTURE}");
    assert!(normalize_world_json(&bom, LiveSourceKind::File, 1, 0, 0).is_ok());
    let torn = &FIXTURE[..FIXTURE.len() / 2];
    assert!(matches!(normalize_world_json(torn, LiveSourceKind::File, 1, 0, 0), Err(NormalizeError::Parse(_))));
    assert!(matches!(normalize_world_json("", LiveSourceKind::File, 1, 0, 0), Err(NormalizeError::Empty)));
}

#[test]
fn drops_actors_without_coordinates_and_leaves_absent_yaw_none() {
    let raw = r#"{"ActorData":[
        {"Type":"Character","UnitType":"WildPal","InstanceID":"a","LocationY":1.0,"LocationZ":2.0},
        {"Type":"PalBox","Name":"Main","LocationX":1114.0,"LocationY":-137219.6,"LocationZ":2886.1}
    ]}"#;
    let f = normalize_world_json(raw, LiveSourceKind::File, 1, 0, 0).unwrap();
    assert_eq!(f.actors.len(), 1, "the coordinate-less actor must be dropped");
    assert_eq!(f.actors[0].kind, "palbox");
    assert_eq!(f.actors[0].yaw, None);
    assert_eq!(f.actors[0].level, None);
}

#[test]
fn species_extracted_from_blueprint_class() {
    let f = normalize_world_json(FIXTURE, LiveSourceKind::File, 1, 0, 0).unwrap();
    let otomo = f.actors.iter().find(|a| a.kind == "otomo").unwrap();
    assert_eq!(otomo.species.as_deref(), Some("JetDragon"));
}
