//! Loading a stored network policy must survive configs saved by older
//! builds. Regression: a legacy row with Tailscale Funnel enabled and
//! authentication off failed validation on boot and killed the server.

#[tokio::test]
async fn legacy_funnel_config_is_clamped_and_persisted_on_load() {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("t.db")).await.unwrap();
    let driver = ps_db::SqlxSqliteDriver::new(pool);

    // The exact shape that bricked startup: Funnel on, auth scope "never"
    // (with a leftover PIN hash, which a "never" scope ignores).
    let legacy = r#"{"version":1,"listen":"localhost","port":5174,"allow":{},"auth":{"scope":"never","pin":{"salt":"fc6c72de16a96745b331fe6f362c5ee6","hash":"bf59d516d19acb38de567920ec73050463e5fd1ec284e2b74bfe13d5663f68b0","iterations":600000},"session_ttl_secs":43200},"upnp_enabled":false,"funnel_enabled":true}"#;
    ps_db::meta::set(&driver, ps_server::network::META_KEY, legacy)
        .await
        .unwrap();

    let runtime = ps_server::network::NetworkRuntime::load(&driver)
        .await
        .expect("a legacy config must load, not kill the server");
    assert!(
        !runtime.effective_config().funnel_enabled,
        "Funnel is dropped rather than run without authentication"
    );

    // The clamped policy is persisted, so the next boot starts clean.
    let stored = ps_db::meta::get(&driver, ps_server::network::META_KEY)
        .await
        .unwrap()
        .expect("config row present");
    assert!(stored.contains("\"funnel_enabled\":false"));
    ps_server::network::NetworkRuntime::load(&driver)
        .await
        .expect("reloading the persisted policy stays healthy");
}
