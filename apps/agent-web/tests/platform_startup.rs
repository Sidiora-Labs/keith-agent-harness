use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use keith_agent_web::{WebServer, WebServerConfig};
use keith_credentials::MasterKey;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn platform_web_startup_serves_the_login_shell() {
    let directory = tempfile::tempdir().unwrap();
    let assets = directory.path().join("assets");
    std::fs::create_dir(&assets).unwrap();
    std::fs::write(assets.join("agent_web.js"), b"export default function(){}").unwrap();
    std::fs::write(assets.join("agent_web_bg.wasm"), b"\0asm\x01\0\0\0").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = WebServer::new(WebServerConfig {
        bind: address,
        exact_origin: format!("http://{address}"),
        daemon_socket: PathBuf::from("platform-startup.sock"),
        asset_root: assets,
        credential_root: directory.path().join("credentials"),
        credential_key: MasterKey::from_bytes([0x31; 32]),
        login_secret: b"platform-login-secret".to_vec(),
        session_lifetime: Duration::from_secs(60),
        mutation_limit_per_second: 8,
        daemon_timeout: Duration::from_secs(1),
    })
    .unwrap();
    let task = tokio::spawn(server.serve_listener(listener));
    let response = tokio::task::spawn_blocking(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(b"GET /login HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    })
    .await
    .unwrap();
    task.abort();
    let _ = task.await;
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("Keith Agent sign in"));
}
