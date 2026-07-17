#[derive(serde::Deserialize)]
struct Vector {
    steam_id: u64,
    palworld_uid: String,
    nosteam_uid: String,
}

#[test]
fn conversions_match_python_reference_vectors() {
    let raw = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/steam_id_vectors.json"),
    )
    .unwrap();
    let vectors: Vec<Vector> = serde_json::from_str(&raw).unwrap();
    assert!(!vectors.is_empty());
    for vector in vectors {
        let uid = psp_core::steam_id::steam_id_to_player_uid(vector.steam_id);
        assert_eq!(
            uid.to_string(),
            vector.palworld_uid,
            "steam_id {}",
            vector.steam_id
        );
        assert_eq!(
            psp_core::steam_id::player_uid_to_nosteam(uid),
            vector.nosteam_uid
        );
    }
}

#[test]
fn input_parsing_accepts_all_supported_formats() {
    assert_eq!(
        psp_core::steam_id::parse_steam_input(
            "https://steamcommunity.com/profiles/76561198000000001/"
        )
        .unwrap(),
        76561198000000001
    );
    assert_eq!(
        psp_core::steam_id::parse_steam_input("steam_42").unwrap(),
        42
    );
    assert!(matches!(
        psp_core::steam_id::parse_steam_input("https://steamcommunity.com/id/somebody"),
        Err(psp_core::steam_id::SteamIdError::VanityUrl)
    ));
    // The error message is the wire contract: it quotes the PROCESSED string
    // (after prefix/URL stripping), not the raw input.
    assert_eq!(
        psp_core::steam_id::parse_steam_input("garbage!!")
            .unwrap_err()
            .to_string(),
        "invalid literal for int() with base 10: 'garbage!!'"
    );
    assert_eq!(
        psp_core::steam_id::parse_steam_input("steam_abc")
            .unwrap_err()
            .to_string(),
        "invalid literal for int() with base 10: 'abc'"
    );
    assert!(psp_core::steam_id::is_palworld_uid(
        "AABBCCDD000000000000000000000000"
    ));
    assert!(psp_core::steam_id::is_palworld_uid(
        "aabbccdd-0000-0000-0000-000000000000"
    ));
    assert!(!psp_core::steam_id::is_palworld_uid("not-a-uid"));
    assert_eq!(
        psp_core::steam_id::parse_palworld_uid("AABBCCDD000000000000000000000000")
            .unwrap()
            .to_string(),
        "aabbccdd-0000-0000-0000-000000000000"
    );
}
