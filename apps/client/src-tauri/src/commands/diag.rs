use opencade_networking::{NatMapping, UdpPeer, discover_reflexive_address};
use serde::Serialize;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

#[derive(Debug, Serialize)]
pub struct NetworkDiagnostics {
    pub nat: String,
    pub rtt_ms: Option<u64>,
    pub loss: f32,
    pub jitter_ms: f32,
    pub relay_reachable: bool,
    pub stun_reachable: bool,
}

async fn configured_stun_server() -> Option<SocketAddr> {
    let endpoint = match std::env::var("OPENCADE_STUN_SERVER") {
        Ok(endpoint) => endpoint,
        Err(_) => {
            let host = std::env::var("STUN_HOST").ok()?;
            let port = std::env::var("STUN_PORT")
                .ok()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(3478);
            format!("{host}:{port}")
        }
    };
    tokio::net::lookup_host(endpoint).await.ok()?.next()
}

#[tauri::command]
pub async fn network_test() -> NetworkDiagnostics {
    let started = Instant::now();
    let server = std::env::var("OPENCADE_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let relay_reachable = matches!(
        tokio::time::timeout(
            Duration::from_secs(1),
            tokio::net::TcpStream::connect(server),
        )
        .await,
        Ok(Ok(_))
    );

    let stun_server = configured_stun_server().await;
    let (nat, stun_rtt_ms, stun_reachable) = match stun_server {
        None => ("unknown".to_string(), None, false),
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
                        Some(
                            stun_started
                                .elapsed()
                                .as_millis()
                                .try_into()
                                .unwrap_or(u64::MAX),
                        ),
                        true,
                    ),
                    Err(_) => ("blocked".to_string(), None, false),
                },
                Err(_) => ("blocked".to_string(), None, false),
            }
        }
    };

    let relay_rtt_ms =
        relay_reachable.then(|| started.elapsed().as_millis().try_into().unwrap_or(u64::MAX));
    NetworkDiagnostics {
        nat,
        rtt_ms: stun_rtt_ms.or(relay_rtt_ms),
        loss: 0.0,
        jitter_ms: 0.0,
        relay_reachable,
        stun_reachable,
    }
}
