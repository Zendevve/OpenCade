use opencade_networking::{NatMapping, UdpPeer, discover_reflexive_address};
use serde::Serialize;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Serialize)]
pub struct NetworkDiagnostics {
    pub nat: String,
    pub rtt_ms: Option<u128>,
    pub relay_reachable: bool,
}

#[tauri::command]
pub async fn network_test() -> NetworkDiagnostics {
    let started = Instant::now();
    let server = std::env::var("OPENCADE_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let reachable = tokio::net::TcpStream::connect(server).await.is_ok();
    let stun_server = std::env::var("OPENCADE_STUN_SERVER")
        .ok()
        .and_then(|value| value.parse::<SocketAddr>().ok());
    let (nat, rtt_ms) = match stun_server {
        None => ("unknown".to_string(), None),
        Some(stun_server) => {
            let stun_started = Instant::now();
            match UdpPeer::bind_unconnected(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).await {
                Ok(peer) => match discover_reflexive_address(
                    &peer,
                    stun_server,
                    Duration::from_millis(1_500),
                )
                .await
                {
                    Ok(observation) => (
                        match observation.mapping {
                            NatMapping::Open => "open",
                            NatMapping::Mapped => "mapped",
                        }
                        .to_string(),
                        Some(stun_started.elapsed().as_millis()),
                    ),
                    Err(_) => ("blocked".to_string(), None),
                },
                Err(_) => ("blocked".to_string(), None),
            }
        }
    };
    NetworkDiagnostics {
        nat,
        rtt_ms: rtt_ms.or_else(|| reachable.then(|| started.elapsed().as_millis())),
        relay_reachable: reachable,
    }
}
