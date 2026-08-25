use super::{InputFrame, MAX_INPUT_BYTES, RelayPeer, TransportError, UdpPeer};
use opencade_emulator_sdk::{MatchDescriptor, PeerRole, TransportKind};
use opencade_protocol::{MatchCandidateKind, NatMappingState};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const MAX_PROBE_FRAMES: u64 = 10_000;
const RETRY_INTERVAL: Duration = Duration::from_millis(25);
// Keep answering lagging final-frame retries after the local transcript completes. A full second
// covers process-start and scheduler skew observed on shared CI runners without extending the
// bounded match deadline or changing the deterministic transcript.
const COMPLETION_GRACE: Duration = Duration::from_secs(1);

// Kept private to the crate so the proof protocol can evolve independently of the data-plane API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ProbePacket {
    Input {
        room_id: String,
        session_key: String,
        frame: InputFrame,
    },
    HolePunch {
        version: u8,
        room_id: String,
        session_key: String,
    },
}

#[derive(Debug, Clone)]
pub struct MatchProbeConfig {
    descriptor: MatchDescriptor,
    session_key: String,
    frame_count: u64,
    timeout: Duration,
}

impl MatchProbeConfig {
    pub fn new(
        descriptor: MatchDescriptor,
        session_key: impl Into<String>,
        frame_count: u64,
        timeout: Duration,
    ) -> Result<Self, TransportError> {
        descriptor
            .validate()
            .map_err(|error| TransportError::InvalidConfiguration(error.to_string()))?;
        if !matches!(
            descriptor.transport,
            TransportKind::DirectUdp | TransportKind::Relay
        ) {
            return Err(TransportError::InvalidConfiguration(
                "match probe requires direct_udp or relay transport".into(),
            ));
        }
        let session_key = session_key.into();
        if session_key.trim().is_empty() || session_key.len() > 128 {
            return Err(TransportError::InvalidConfiguration(
                "session key must contain between 1 and 128 bytes".into(),
            ));
        }
        if !(1..=MAX_PROBE_FRAMES).contains(&frame_count) {
            return Err(TransportError::InvalidConfiguration(format!(
                "frame count must be between 1 and {MAX_PROBE_FRAMES}"
            )));
        }
        if timeout.is_zero() {
            return Err(TransportError::InvalidConfiguration(
                "timeout must be greater than zero".into(),
            ));
        }
        Ok(Self {
            descriptor,
            session_key,
            frame_count,
            timeout,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchProbeReport {
    pub room_id: String,
    pub local_user_id: String,
    pub peer_user_id: String,
    pub role: PeerRole,
    pub transport: TransportKind,
    pub frames_sent: u64,
    pub frames_received: u64,
    pub transcript_checksum: String,
    pub elapsed_ms: u128,
    pub nat: NatMappingState,
    pub candidate: MatchCandidateKind,
    pub punch_attempts: u8,
}

/// Runs a bounded deterministic frame exchange over a connected UDP peer.
///
/// A peer repeats its current frame until the opposite frame arrives. Receiving an older frame
/// causes the matching local frame to be retransmitted, allowing either process to start first.
pub async fn run_match_probe(
    peer: &UdpPeer,
    config: &MatchProbeConfig,
) -> Result<MatchProbeReport, TransportError> {
    run_match_probe_with(peer, config).await
}

pub async fn run_relay_match_probe(
    peer: &RelayPeer,
    config: &MatchProbeConfig,
) -> Result<MatchProbeReport, TransportError> {
    run_match_probe_with(peer, config).await
}

trait ProbeTransport {
    async fn send_packet(&self, packet: &ProbePacket) -> Result<(), TransportError>;
    async fn receive_packet(&self) -> Result<ProbePacket, TransportError>;
}

impl ProbeTransport for UdpPeer {
    async fn send_packet(&self, packet: &ProbePacket) -> Result<(), TransportError> {
        self.send_packet(packet).await
    }

    async fn receive_packet(&self) -> Result<ProbePacket, TransportError> {
        self.receive_packet().await
    }
}

impl ProbeTransport for RelayPeer {
    async fn send_packet(&self, packet: &ProbePacket) -> Result<(), TransportError> {
        self.send_packet(packet).await
    }

    async fn receive_packet(&self) -> Result<ProbePacket, TransportError> {
        self.receive_packet().await
    }
}

async fn run_match_probe_with<T: ProbeTransport>(
    peer: &T,
    config: &MatchProbeConfig,
) -> Result<MatchProbeReport, TransportError> {
    let started = Instant::now();
    let deadline = started + config.timeout;
    let mut checksum = FNV_OFFSET_BASIS;
    let mut frames_sent = 0;

    for frame_number in 0..config.frame_count {
        let local_frame = deterministic_frame(
            frame_number,
            &config.descriptor.local_user_id,
            config.descriptor.role,
        )?;
        send_probe_frame(peer, config, local_frame.clone()).await?;
        frames_sent += 1;

        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(TransportError::Timeout {
                    received_frames: frame_number,
                    expected_frames: config.frame_count,
                });
            }
            let wait = RETRY_INTERVAL.min(deadline.saturating_duration_since(now));
            match tokio::time::timeout(wait, peer.receive_packet()).await {
                Err(_) => {
                    send_probe_frame(peer, config, local_frame.clone()).await?;
                    frames_sent += 1;
                }
                Ok(Err(TransportError::PeerUnavailable)) => {
                    tokio::time::sleep(wait).await;
                    send_probe_frame(peer, config, local_frame.clone()).await?;
                    frames_sent += 1;
                }
                Ok(Err(error)) => return Err(error),
                Ok(Ok(packet)) => {
                    let Some(remote) = validated_remote_frame(packet, config)? else {
                        continue;
                    };
                    if remote.frame < frame_number {
                        let previous = deterministic_frame(
                            remote.frame,
                            &config.descriptor.local_user_id,
                            config.descriptor.role,
                        )?;
                        send_probe_frame(peer, config, previous).await?;
                        frames_sent += 1;
                        continue;
                    }
                    if remote.frame > frame_number {
                        send_probe_frame(peer, config, local_frame.clone()).await?;
                        frames_sent += 1;
                        continue;
                    }
                    update_transcript_checksum(
                        &mut checksum,
                        config.descriptor.role,
                        &local_frame,
                        &remote,
                    );
                    break;
                }
            }
        }
    }

    linger_for_peer(peer, config, &mut frames_sent).await?;
    Ok(MatchProbeReport {
        room_id: config.descriptor.room_id.clone(),
        local_user_id: config.descriptor.local_user_id.clone(),
        peer_user_id: config.descriptor.peer_user_id.clone(),
        role: config.descriptor.role,
        transport: config.descriptor.transport,
        frames_sent,
        frames_received: config.frame_count,
        transcript_checksum: format!("{checksum:016x}"),
        elapsed_ms: started.elapsed().as_millis(),
        nat: NatMappingState::Unknown,
        candidate: MatchCandidateKind::Host,
        punch_attempts: 0,
    })
}

async fn send_probe_frame<T: ProbeTransport>(
    peer: &T,
    config: &MatchProbeConfig,
    frame: InputFrame,
) -> Result<(), TransportError> {
    match peer
        .send_packet(&ProbePacket::Input {
            room_id: config.descriptor.room_id.clone(),
            session_key: config.session_key.clone(),
            frame,
        })
        .await
    {
        // A connected UDP socket can receive an ICMP port-unreachable response while the other
        // process is still binding. The probe's bounded retry loop treats that startup race like
        // a dropped datagram instead of failing the match immediately.
        Err(TransportError::PeerUnavailable) => Ok(()),
        result => result,
    }
}

fn validated_remote_frame(
    packet: ProbePacket,
    config: &MatchProbeConfig,
) -> Result<Option<InputFrame>, TransportError> {
    let ProbePacket::Input {
        room_id,
        session_key,
        frame,
    } = packet
    else {
        return Ok(None);
    };
    if room_id != config.descriptor.room_id
        || session_key != config.session_key
        || frame.player_id != config.descriptor.peer_user_id
        || frame.input.len() > MAX_INPUT_BYTES
    {
        return Err(TransportError::PeerMismatch);
    }
    Ok(Some(frame))
}

async fn linger_for_peer<T: ProbeTransport>(
    peer: &T,
    config: &MatchProbeConfig,
    frames_sent: &mut u64,
) -> Result<(), TransportError> {
    let final_frame_number = config.frame_count - 1;
    let final_frame = deterministic_frame(
        final_frame_number,
        &config.descriptor.local_user_id,
        config.descriptor.role,
    )?;
    let deadline = Instant::now() + COMPLETION_GRACE;
    while Instant::now() < deadline {
        let wait = RETRY_INTERVAL.min(deadline.saturating_duration_since(Instant::now()));
        match tokio::time::timeout(wait, peer.receive_packet()).await {
            Err(_) => {
                send_probe_frame(peer, config, final_frame.clone()).await?;
                *frames_sent += 1;
            }
            Ok(Err(TransportError::PeerUnavailable)) => {
                tokio::time::sleep(wait).await;
                send_probe_frame(peer, config, final_frame.clone()).await?;
                *frames_sent += 1;
            }
            Ok(Err(error)) => return Err(error),
            Ok(Ok(packet)) => {
                let Some(remote) = validated_remote_frame(packet, config)? else {
                    continue;
                };
                let response = deterministic_frame(
                    remote.frame.min(final_frame_number),
                    &config.descriptor.local_user_id,
                    config.descriptor.role,
                )?;
                send_probe_frame(peer, config, response).await?;
                *frames_sent += 1;
            }
        }
    }
    Ok(())
}

fn deterministic_frame(
    frame: u64,
    player_id: &str,
    role: PeerRole,
) -> Result<InputFrame, TransportError> {
    let input = match role {
        PeerRole::Host => (frame % 4) as u8,
        PeerRole::Guest => ((frame + 2) % 4) as u8,
    };
    InputFrame::new(frame, player_id, vec![input])
}

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn update_transcript_checksum(
    checksum: &mut u64,
    role: PeerRole,
    local: &InputFrame,
    remote: &InputFrame,
) {
    let (host, guest) = match role {
        PeerRole::Host => (local, remote),
        PeerRole::Guest => (remote, local),
    };
    for byte in host.input.iter().chain(&guest.input) {
        *checksum ^= u64::from(*byte);
        *checksum = checksum.wrapping_mul(FNV_PRIME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn config() -> MatchProbeConfig {
        MatchProbeConfig::new(
            MatchDescriptor {
                room_id: "room-1".into(),
                game_id: "sfiii3".into(),
                local_user_id: "host".into(),
                peer_user_id: "guest".into(),
                role: PeerRole::Host,
                transport: TransportKind::DirectUdp,
                local_endpoint: "127.0.0.1:41000"
                    .parse::<SocketAddr>()
                    .expect("local endpoint"),
                peer_endpoint: "127.0.0.1:41001"
                    .parse::<SocketAddr>()
                    .expect("peer endpoint"),
                input_delay_frames: 2,
            },
            "session-1",
            60,
            Duration::from_secs(1),
        )
        .expect("probe config")
    }

    #[test]
    fn rejects_packets_from_another_room_session_or_player() {
        let config = config();
        let valid_frame = InputFrame::new(0, "guest", vec![2]).expect("frame");
        let wrong_room = ProbePacket::Input {
            room_id: "room-2".into(),
            session_key: "session-1".into(),
            frame: valid_frame.clone(),
        };
        let wrong_session = ProbePacket::Input {
            room_id: "room-1".into(),
            session_key: "session-2".into(),
            frame: valid_frame,
        };
        let wrong_player = ProbePacket::Input {
            room_id: "room-1".into(),
            session_key: "session-1".into(),
            frame: InputFrame::new(0, "intruder", vec![2]).expect("frame"),
        };

        assert_eq!(
            validated_remote_frame(wrong_room, &config),
            Err(TransportError::PeerMismatch)
        );
        assert_eq!(
            validated_remote_frame(wrong_session, &config),
            Err(TransportError::PeerMismatch)
        );
        assert_eq!(
            validated_remote_frame(wrong_player, &config),
            Err(TransportError::PeerMismatch)
        );
    }

    #[test]
    fn configuration_rejects_empty_and_oversized_probe_values() {
        let descriptor = config().descriptor;
        let zero_frames =
            MatchProbeConfig::new(descriptor.clone(), "session-1", 0, Duration::from_secs(1));
        assert!(matches!(
            zero_frames,
            Err(TransportError::InvalidConfiguration(_))
        ));

        let oversized_key =
            MatchProbeConfig::new(descriptor, "x".repeat(129), 1, Duration::from_secs(1));
        assert!(matches!(
            oversized_key,
            Err(TransportError::InvalidConfiguration(_))
        ));
    }
}
