use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use psp_server::{start_server, ServerConfig};

// Port 0 picks an ephemeral port, so this can run concurrently with other tests.
#[tokio::test]
async fn start_server_binds_the_port_before_returning() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("ui")).unwrap();
    std::fs::write(temp_dir.path().join("ui/index.html"), "<html>psp</html>").unwrap();

    let config = ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir: temp_dir.path().join("ui"),
        data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data"),
        db_path: temp_dir.path().join("test.db"),
        desktop_mode: false,
    };

    let handle = start_server(config).await.unwrap();

    // No retry loop, no sleep: this connects immediately.
    let mut stream = TcpStream::connect(handle.addr)
        .await
        .expect("port must already be bound and listening when start_server returns");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "expected a real HTTP response from the freshly bound server, got: {response}"
    );

    handle.shutdown().await;
}

#[tokio::test]
async fn shutdown_completes_and_frees_the_port() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("ui")).unwrap();
    std::fs::write(temp_dir.path().join("ui/index.html"), "<html>psp</html>").unwrap();

    let config = ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir: temp_dir.path().join("ui"),
        data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data"),
        db_path: temp_dir.path().join("test.db"),
        desktop_mode: false,
    };

    let handle = start_server(config).await.unwrap();
    let addr = handle.addr;
    handle.shutdown().await;

    // Fails with AddrInUse if the old listener is still alive.
    let rebound = tokio::net::TcpListener::bind(addr).await;
    assert!(rebound.is_ok(), "port {addr} was not freed after shutdown");
}
