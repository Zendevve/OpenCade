use serde::Serialize;
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NatType {
    Open,
    Cone,
    Symmetric,
    Blocked,
    #[default]
    Unknown,
}

impl NatType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Cone => "cone",
            Self::Symmetric => "symmetric",
            Self::Blocked => "blocked",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for NatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Serialize)]
pub struct NetworkDiagnostics {
    pub nat: String,
    pub rtt_ms: Option<u64>,
    pub loss: f32,
    pub jitter_ms: f32,
    pub relay_reachable: bool,
    pub stun_reachable: bool,
}

#[must_use]
pub fn classify_nat(rtt_ok: bool, stun_ok: bool) -> &'static str {
    let _ = rtt_ok;
    if stun_ok {
        NatType::Cone.as_str()
    } else {
        NatType::Unknown.as_str()
    }
}

#[tauri::command]
pub async fn network_test() -> NetworkDiagnostics {
    let started = Instant::now();
    let relay_reachable = matches!(
        tokio::time::timeout(
            Duration::from_secs(1),
            tokio::net::TcpStream::connect("127.0.0.1:8080"),
        )
        .await,
        Ok(Ok(_))
    );
    let rtt_ms = if relay_reachable {
        Some(started.elapsed().as_millis() as u64)
    } else {
        None
    };
    let rtt_ok = relay_reachable;

    let stun_host =
        std::env::var("STUN_HOST").unwrap_or_else(|_| "stun.opencade.local:3478".to_string());
    let stun_reachable = matches!(
        tokio::time::timeout(
            Duration::from_millis(200),
            tokio::net::TcpStream::connect(&stun_host),
        )
        .await,
        Ok(Ok(_))
    );

    let nat = classify_nat(rtt_ok, stun_reachable).to_string();

    // Loss and jitter are measured over a longer window in the full networking stack;
    // for this lightweight TCP probe they are reported as zero but the fields are
    // kept stable for the UI contract.
    let loss = 0.0_f32;
    let jitter_ms = 0.0_f32;

    NetworkDiagnostics {
        nat,
        rtt_ms,
        loss,
        jitter_ms,
        relay_reachable,
        stun_reachable,
    }
}
