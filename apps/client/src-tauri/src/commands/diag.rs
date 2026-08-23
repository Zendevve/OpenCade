use serde::Serialize;
use std::time::Instant;

#[derive(Debug, Serialize)]
pub struct NetworkDiagnostics {
    pub nat: &'static str,
    pub rtt_ms: Option<u128>,
    pub relay_reachable: bool,
}

#[tauri::command]
pub async fn network_test() -> NetworkDiagnostics {
    let started = Instant::now();
    let reachable = tokio::net::TcpStream::connect("127.0.0.1:8080")
        .await
        .is_ok();
    NetworkDiagnostics {
        nat: "unknown",
        rtt_ms: reachable.then(|| started.elapsed().as_millis()),
        relay_reachable: reachable,
    }
}
