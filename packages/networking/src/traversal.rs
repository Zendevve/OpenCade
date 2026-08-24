use crate::{TransportError, UdpPeer};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;

const MAX_PUNCH_CANDIDATES: usize = 8;
const MAX_PUNCH_BYTES: usize = 1_024;

#[derive(Debug, Clone)]
pub struct HolePunchConfig {
    room_id: String,
    session_key: String,
    peer_candidates: Vec<SocketAddr>,
    attempts: u8,
    interval: Duration,
}

impl HolePunchConfig {
    pub fn new(
        room_id: impl Into<String>,
        session_key: impl Into<String>,
        peer_candidates: Vec<SocketAddr>,
        attempts: u8,
        interval: Duration,
    ) -> Result<Self, TransportError> {
        let room_id = room_id.into();
        let session_key = session_key.into();
        let mut seen = HashSet::new();
        let peer_candidates = peer_candidates
            .into_iter()
            .filter(|candidate| seen.insert(*candidate))
            .collect::<Vec<_>>();
        if room_id.trim().is_empty()
            || room_id.len() > 128
            || session_key.trim().is_empty()
            || session_key.len() > 128
        {
            return Err(TransportError::InvalidConfiguration(
                "hole punch requires bounded room and session identifiers".into(),
            ));
        }
        if peer_candidates.is_empty()
            || peer_candidates.len() > MAX_PUNCH_CANDIDATES
            || peer_candidates
                .iter()
                .any(|candidate| candidate.ip().is_unspecified() || candidate.port() == 0)
        {
            return Err(TransportError::InvalidConfiguration(format!(
                "hole punch requires 1 to {MAX_PUNCH_CANDIDATES} usable peer candidates"
            )));
        }
        if attempts == 0 || attempts > 10 || interval.is_zero() {
            return Err(TransportError::InvalidConfiguration(
                "hole punch requires 1 to 10 attempts and a non-zero interval".into(),
            ));
        }
        Ok(Self {
            room_id,
            session_key,
            peer_candidates,
            attempts,
            interval,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HolePunchReport {
    pub selected_peer: SocketAddr,
    pub attempts: u8,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PunchPacket {
    HolePunch {
        version: u8,
        room_id: String,
        session_key: String,
    },
}

/// Simultaneously sends nonce-bound packets to every advertised peer candidate, then connects
/// the reserved socket to the first valid responder. The operation is strictly bounded.
pub async fn punch_hole(
    peer: UdpPeer,
    config: &HolePunchConfig,
) -> Result<(UdpPeer, HolePunchReport), TransportError> {
    let packet = serde_json::to_vec(&PunchPacket::HolePunch {
        version: 1,
        room_id: config.room_id.clone(),
        session_key: config.session_key.clone(),
    })
    .map_err(|error| TransportError::Serialization(error.to_string()))?;
    let candidates = config
        .peer_candidates
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut buffer = [0_u8; MAX_PUNCH_BYTES];

    for attempt in 1..=config.attempts {
        for candidate in &config.peer_candidates {
            match peer.socket.send_to(&packet, candidate).await {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {}
                Err(error) => return Err(super::map_udp_error(error)),
            }
        }
        let deadline = tokio::time::Instant::now() + config.interval;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let received = match tokio::time::timeout(remaining, peer.socket.recv_from(&mut buffer))
                .await
            {
                Err(_) => break,
                Ok(Err(error)) if matches!(error.kind(), std::io::ErrorKind::ConnectionRefused) => {
                    continue;
                }
                Ok(Err(error)) => return Err(super::map_udp_error(error)),
                Ok(Ok(received)) => received,
            };
            if !candidates.contains(&received.1) {
                continue;
            }
            let Ok(candidate_packet) = serde_json::from_slice::<PunchPacket>(&buffer[..received.0])
            else {
                continue;
            };
            let PunchPacket::HolePunch {
                version,
                room_id,
                session_key,
            } = candidate_packet;
            if version != 1 || room_id != config.room_id || session_key != config.session_key {
                continue;
            }
            peer.socket
                .send_to(&packet, received.1)
                .await
                .map_err(super::map_udp_error)?;
            let connected = peer.connect(received.1).await?;
            return Ok((
                connected,
                HolePunchReport {
                    selected_peer: received.1,
                    attempts: attempt,
                },
            ));
        }
    }
    Err(TransportError::HolePunchTimeout {
        attempts: config.attempts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MatchProbeConfig, run_match_probe};
    use opencade_emulator_sdk::{MatchDescriptor, PeerRole, TransportKind};

    #[tokio::test]
    async fn loopback_peers_punch_and_select_each_other() {
        let host = UdpPeer::bind_unconnected("127.0.0.1:0".parse().expect("host bind"))
            .await
            .expect("host peer");
        let guest = UdpPeer::bind_unconnected("127.0.0.1:0".parse().expect("guest bind"))
            .await
            .expect("guest peer");
        let host_address = host.local_addr().expect("host address");
        let guest_address = guest.local_addr().expect("guest address");
        let host_config = HolePunchConfig::new(
            "room-1",
            "session-1",
            vec![guest_address],
            3,
            Duration::from_millis(100),
        )
        .expect("host config");
        let guest_config = HolePunchConfig::new(
            "room-1",
            "session-1",
            vec![host_address],
            3,
            Duration::from_millis(100),
        )
        .expect("guest config");

        let (host_result, guest_result) = tokio::join!(
            punch_hole(host, &host_config),
            punch_hole(guest, &guest_config)
        );
        assert_eq!(
            host_result.expect("host punch").1.selected_peer,
            guest_address
        );
        assert_eq!(
            guest_result.expect("guest punch").1.selected_peer,
            host_address
        );
    }

    #[tokio::test]
    async fn punched_peers_continue_into_the_deterministic_probe() {
        let host = UdpPeer::bind_unconnected("127.0.0.1:0".parse().expect("host bind"))
            .await
            .expect("host peer");
        let guest = UdpPeer::bind_unconnected("127.0.0.1:0".parse().expect("guest bind"))
            .await
            .expect("guest peer");
        let host_address = host.local_addr().expect("host address");
        let guest_address = guest.local_addr().expect("guest address");
        let host_punch = HolePunchConfig::new(
            "room-1",
            "session-1",
            vec![guest_address],
            3,
            Duration::from_millis(100),
        )
        .expect("host punch config");
        let guest_punch = HolePunchConfig::new(
            "room-1",
            "session-1",
            vec![host_address],
            3,
            Duration::from_millis(100),
        )
        .expect("guest punch config");
        let (host_result, guest_result) = tokio::join!(
            punch_hole(host, &host_punch),
            punch_hole(guest, &guest_punch)
        );
        let (host, _) = host_result.expect("host punch");
        let (guest, _) = guest_result.expect("guest punch");

        let config = |role, local_user: &str, peer_user: &str, local, remote| {
            MatchProbeConfig::new(
                MatchDescriptor {
                    room_id: "room-1".into(),
                    game_id: "sfiii3".into(),
                    local_user_id: local_user.into(),
                    peer_user_id: peer_user.into(),
                    role,
                    transport: TransportKind::DirectUdp,
                    local_endpoint: local,
                    peer_endpoint: remote,
                    input_delay_frames: 2,
                },
                "session-1",
                60,
                Duration::from_secs(3),
            )
            .expect("probe config")
        };
        let host_config = config(PeerRole::Host, "host", "guest", host_address, guest_address);
        let guest_config = config(
            PeerRole::Guest,
            "guest",
            "host",
            guest_address,
            host_address,
        );
        let (host_report, guest_report) = tokio::join!(
            run_match_probe(&host, &host_config),
            run_match_probe(&guest, &guest_config)
        );
        assert_eq!(
            host_report.expect("host probe").transcript_checksum,
            guest_report.expect("guest probe").transcript_checksum
        );
    }

    #[test]
    fn rejects_unbounded_or_unusable_candidates() {
        assert!(
            HolePunchConfig::new("room", "session", Vec::new(), 3, Duration::from_millis(500))
                .is_err()
        );
        assert!(
            HolePunchConfig::new(
                "room",
                "session",
                vec!["0.0.0.0:0".parse().expect("unspecified")],
                3,
                Duration::from_millis(500)
            )
            .is_err()
        );
    }
}
