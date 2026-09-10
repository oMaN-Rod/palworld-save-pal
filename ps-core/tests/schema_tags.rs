/// `ensure_schema` no-ops on every path a save already records, so comparing a primed
/// real save against itself proves nothing. Prime a save with an EMPTY schema table
/// instead, then compare the invented tags against what real saves actually record.
#[test]
fn invented_tags_match_the_tags_real_saves_record() {
    let mut primed = ps_core::ue::Save {
        header: ps_core::ue::Header {
            magic: 0,
            save_game_version: 0,
            package_version: ps_core::ue::PackageVersion { ue4: 0, ue5: None },
            engine_version_major: 0,
            engine_version_minor: 0,
            engine_version_patch: 0,
            engine_version_build: 0,
            engine_version: String::new(),
            custom_version: None,
        },
        schemas: ps_core::ue::PropertySchemas::default(),
        root: ps_core::ue::Root {
            save_game_type: String::new(),
            properties: ps_core::ue::Properties::default(),
        },
        extra: Vec::new(),
    };

    ps_core::domain::pal::ensure_pal_property_schemas(&mut primed);
    ps_core::domain::containers::ensure_container_schemas(&mut primed);

    let mut mismatches = Vec::new();
    let mut compared = 0;
    for name in ["world1", "v1_stats", "v1_relics"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves")
            .join(name)
            .join("Level.sav");
        let bytes = std::fs::read(&path).expect("read");
        let real = ps_core::savio::read_sav_bytes(&bytes).expect("parse");

        for (schema_path, invented) in primed.schemas.schemas() {
            if let Some(recorded) = real.schemas.get(schema_path) {
                compared += 1;
                if recorded != invented {
                    mismatches.push(format!(
                        "[{name}] {schema_path}\n    real     = {}\n    invented = {}",
                        serde_json::to_string(recorded).unwrap(),
                        serde_json::to_string(invented).unwrap()
                    ));
                }
            }
        }
    }
    println!("compared {compared} invented tags against real ones");
    assert!(compared > 50, "test is not actually comparing anything");
    assert!(
        mismatches.is_empty(),
        "the primer invents {} tag(s) that disagree with real saves:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
