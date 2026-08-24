use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::sync::mpsc;

pub mod latency;
pub mod nat;

mod probe;
mod relay;
mod report;
mod stun;
mod traversal;

pub use latency::LatencyMetrics;
pub use nat::{
    FALLBACK_ORDER, FallbackOrder, NatTraversal, NatType, next_transport, transport_for_nat,
};
pub use probe::{
    MAX_PROBE_FRAMES, MatchProbeConfig, MatchProbeReport, run_match_probe, run_relay_match_probe,
};
pub use relay::RelayPeer;
pub use report::{
    ALPHA_MATCH_FRAMES, AlphaCampaignEvidence, AlphaCampaignFailure, AlphaCampaignSummary,
    CompatibilityResult, MAX_REPORT_BYTES, MatchVerification, ReportReadError,
    ReportVerificationError, read_campaign_evidence, read_match_report,
    summarize_campaign_evidence, summarize_match_reports, verify_match_reports,
    verify_playable_match_reports,
};
pub use stun::{NatMapping, StunObservation, discover_reflexive_address};
pub use traversal::{HolePunchConfig, HolePunchReport, punch_hole};
pub const MAX_INPUT_BYTES: usize = 256;
pub const INPUT_QUEUE_CAPACITY: usize = 120;
const MAX_DATAGRAM_BYTES: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputFrame {
    pub frame: u64,
    pub player_id: String,
    pub input: Vec<u8>,
}

impl InputFrame {
    pub fn new(
        frame: u64,
        player_id: impl Into<String>,
        input: Vec<u8>,
    ) -> Result<Self, TransportError> {
        let player_id = player_id.into();
        if player_id.trim().is_empty() {
            return Err(TransportError::InvalidPlayer);
        }
        if input.len() > MAX_INPUT_BYTES {
            return Err(TransportError::FrameTooLarge(input.len()));
        }
        Ok(Self {
            frame,
            player_id,
            input,
        })
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TransportError {
    #[error("player id must not be empty")]
    InvalidPlayer,
    #[error("input frame contains {0} bytes; maximum is {MAX_INPUT_BYTES}")]
    FrameTooLarge(usize),
    #[error("peer input queue is full")]
    Backpressure,
    #[error("peer transport is closed")]
    Closed,
    #[error("datagram serialization failed: {0}")]
    Serialization(String),
    #[error("udp transport failed: {0}")]
    Io(String),
    #[error("udp peer is temporarily unavailable")]
    PeerUnavailable,
    #[error("invalid match probe configuration: {0}")]
    InvalidConfiguration(String),
    #[error("received a datagram for a different match or peer")]
    PeerMismatch,
    #[error("match probe timed out after receiving {received_frames} of {expected_frames} frames")]
    Timeout {
        received_frames: u64,
        expected_frames: u64,
    },
    #[error("STUN discovery timed out")]
    StunTimeout,
    #[error("invalid STUN response: {0}")]
    InvalidStunResponse(String),
    #[error("UDP hole punch timed out after {attempts} attempts")]
    HolePunchTimeout { attempts: u8 },
    #[error("relay transport failed: {0}")]
    Relay(String),
}

/// Connected UDP transport for LAN proof runs. Authentication and endpoint negotiation remain in
/// the control plane; this type only carries bounded OpenCade input frames.
pub struct UdpPeer {
    socket: tokio::net::UdpSocket,
}

impl UdpPeer {
    pub async fn bind(local: SocketAddr, peer: SocketAddr) -> Result<Self, TransportError> {
        let socket = Self::bind_unconnected(local).await?;
        socket.connect(peer).await
    }

    pub async fn bind_unconnected(local: SocketAddr) -> Result<Self, TransportError> {
        let socket = tokio::net::UdpSocket::bind(local)
            .await
            .map_err(map_udp_error)?;
        Ok(Self { socket })
    }

    pub async fn connect(self, peer: SocketAddr) -> Result<Self, TransportError> {
        self.socket.connect(peer).await.map_err(map_udp_error)?;
        Ok(self)
    }

    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        self.socket.local_addr().map_err(map_udp_error)
    }

    pub async fn send(&self, frame: &InputFrame) -> Result<(), TransportError> {
        if frame.input.len() > MAX_INPUT_BYTES || frame.player_id.trim().is_empty() {
            return Err(TransportError::Serialization("invalid input frame".into()));
        }
        let encoded = serde_json::to_vec(frame)
            .map_err(|error| TransportError::Serialization(error.to_string()))?;
        self.socket.send(&encoded).await.map_err(map_udp_error)?;
        Ok(())
    }

    pub async fn receive(&self) -> Result<InputFrame, TransportError> {
        let mut buffer = [0_u8; MAX_DATAGRAM_BYTES];
        let received = self.socket.recv(&mut buffer).await.map_err(map_udp_error)?;
        let frame: InputFrame = serde_json::from_slice(&buffer[..received])
            .map_err(|error| TransportError::Serialization(error.to_string()))?;
        InputFrame::new(frame.frame, frame.player_id, frame.input)
    }

    async fn send_packet(&self, packet: &probe::ProbePacket) -> Result<(), TransportError> {
        let encoded = serde_json::to_vec(packet)
            .map_err(|error| TransportError::Serialization(error.to_string()))?;
        if encoded.len() > MAX_DATAGRAM_BYTES {
            return Err(TransportError::Serialization(
                "match probe datagram exceeds maximum size".into(),
            ));
        }
        self.socket.send(&encoded).await.map_err(map_udp_error)?;
        Ok(())
    }

    async fn receive_packet(&self) -> Result<probe::ProbePacket, TransportError> {
        let mut buffer = [0_u8; MAX_DATAGRAM_BYTES];
        let received = self.socket.recv(&mut buffer).await.map_err(map_udp_error)?;
        serde_json::from_slice(&buffer[..received])
            .map_err(|error| TransportError::Serialization(error.to_string()))
    }
}

fn map_udp_error(error: std::io::Error) -> TransportError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::ConnectionReset
    ) {
        TransportError::PeerUnavailable
    } else {
        TransportError::Io(error.to_string())
    }
}

/// Deterministic, bounded transport used to prove the match contract without a network.
pub struct InMemoryPeer {
    outbound: mpsc::Sender<InputFrame>,
    inbound: mpsc::Receiver<InputFrame>,
}

impl InMemoryPeer {
    pub fn pair() -> (Self, Self) {
        let (a_to_b_tx, a_to_b_rx) = mpsc::channel(INPUT_QUEUE_CAPACITY);
        let (b_to_a_tx, b_to_a_rx) = mpsc::channel(INPUT_QUEUE_CAPACITY);
        (
            Self {
                outbound: a_to_b_tx,
                inbound: b_to_a_rx,
            },
            Self {
                outbound: b_to_a_tx,
                inbound: a_to_b_rx,
            },
        )
    }

    pub fn try_send(&self, frame: InputFrame) -> Result<(), TransportError> {
        self.outbound.try_send(frame).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => TransportError::Backpressure,
            mpsc::error::TrySendError::Closed(_) => TransportError::Closed,
        })
    }

    pub async fn receive(&mut self) -> Result<InputFrame, TransportError> {
        self.inbound.recv().await.ok_or(TransportError::Closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencade_emulator_sdk::{
        EmulatorAdapter, MatchDescriptor, MockAdapter, PeerRole, TransportKind,
    };
    use std::time::Duration;

    fn descriptor(role: PeerRole) -> MatchDescriptor {
        let (local_user_id, peer_user_id, local_endpoint, peer_endpoint) = match role {
            PeerRole::Host => ("host", "guest", "127.0.0.1:41000", "127.0.0.1:41001"),
            PeerRole::Guest => ("guest", "host", "127.0.0.1:41001", "127.0.0.1:41000"),
        };
        MatchDescriptor {
            room_id: "proof-room".into(),
            game_id: "sfiii3".into(),
            local_user_id: local_user_id.into(),
            peer_user_id: peer_user_id.into(),
            role,
            transport: TransportKind::InMemory,
            local_endpoint: local_endpoint.parse().expect("local endpoint"),
            peer_endpoint: peer_endpoint.parse().expect("peer endpoint"),
            input_delay_frames: 2,
        }
    }

    #[tokio::test]
    async fn peers_exchange_deterministic_frames_in_both_directions() {
        let (mut host, mut guest) = InMemoryPeer::pair();
        let host_frame = InputFrame::new(7, "host", vec![1, 0, 1]).expect("valid host frame");
        let guest_frame = InputFrame::new(7, "guest", vec![0, 1, 0]).expect("valid guest frame");

        host.try_send(host_frame.clone()).expect("host send");
        guest.try_send(guest_frame.clone()).expect("guest send");

        assert_eq!(guest.receive().await, Ok(host_frame));
        assert_eq!(host.receive().await, Ok(guest_frame));
    }

    #[tokio::test]
    async fn proof_of_match_prepares_opposite_adapters_and_agrees_on_transcript() {
        let host_adapter = MockAdapter::default();
        let guest_adapter = MockAdapter::default();
        host_adapter
            .prepare_match(&descriptor(PeerRole::Host))
            .expect("host match");
        guest_adapter
            .prepare_match(&descriptor(PeerRole::Guest))
            .expect("guest match");
        let (mut host, mut guest) = InMemoryPeer::pair();
        let mut host_transcript = Vec::new();
        let mut guest_transcript = Vec::new();

        for frame in 0..60 {
            let host_frame =
                InputFrame::new(frame, "host", vec![(frame % 4) as u8]).expect("host input");
            let guest_frame = InputFrame::new(frame, "guest", vec![((frame + 2) % 4) as u8])
                .expect("guest input");
            host.try_send(host_frame.clone()).expect("host send");
            guest.try_send(guest_frame.clone()).expect("guest send");
            let at_host = host.receive().await.expect("host receive");
            let at_guest = guest.receive().await.expect("guest receive");
            host_transcript.push((host_frame, at_host));
            guest_transcript.push((at_guest, guest_frame));
        }

        assert_eq!(host_transcript, guest_transcript);
        assert_eq!(
            host_adapter.prepared_matches().expect("host transcript")[0].room_id,
            "proof-room"
        );
        assert_eq!(
            guest_adapter.prepared_matches().expect("guest transcript")[0].role,
            PeerRole::Guest
        );
    }

    #[test]
    fn rejects_oversized_frames() {
        let error = InputFrame::new(0, "host", vec![0; MAX_INPUT_BYTES + 1]);
        assert_eq!(error, Err(TransportError::FrameTooLarge(257)));
    }

    #[test]
    fn connected_udp_icmp_errors_are_retryable_startup_races() {
        for kind in [
            std::io::ErrorKind::ConnectionRefused,
            std::io::ErrorKind::ConnectionReset,
        ] {
            assert_eq!(
                map_udp_error(std::io::Error::from(kind)),
                TransportError::PeerUnavailable
            );
        }
    }

    #[test]
    fn bounded_queue_applies_backpressure() {
        let (host, _guest) = InMemoryPeer::pair();
        for frame in 0..INPUT_QUEUE_CAPACITY {
            host.try_send(InputFrame::new(frame as u64, "host", vec![]).expect("valid frame"))
                .expect("queue has capacity");
        }
        let overflow = InputFrame::new(INPUT_QUEUE_CAPACITY as u64, "host", vec![])
            .expect("valid overflow frame");
        assert_eq!(host.try_send(overflow), Err(TransportError::Backpressure));
    }

    #[tokio::test]
    async fn udp_peers_exchange_a_frame_on_loopback() {
        let reserve_a = tokio::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("reserve a");
        let reserve_b = tokio::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("reserve b");
        let address_a = reserve_a.local_addr().expect("address a");
        let address_b = reserve_b.local_addr().expect("address b");
        drop(reserve_a);
        drop(reserve_b);
        let peer_a = UdpPeer::bind(address_a, address_b).await.expect("peer a");
        let peer_b = UdpPeer::bind(address_b, address_a).await.expect("peer b");
        let expected = InputFrame::new(42, "host", vec![1, 2, 3]).expect("frame");
        peer_a.send(&expected).await.expect("udp send");
        let received = tokio::time::timeout(std::time::Duration::from_secs(1), peer_b.receive())
            .await
            .expect("receive timeout")
            .expect("udp receive");
        assert_eq!(received, expected);
    }

    #[tokio::test]
    async fn udp_match_probes_agree_on_a_deterministic_transcript() {
        let reserve_host = tokio::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("reserve host");
        let reserve_guest = tokio::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("reserve guest");
        let host_address = reserve_host.local_addr().expect("host address");
        let guest_address = reserve_guest.local_addr().expect("guest address");
        drop(reserve_host);
        drop(reserve_guest);

        let host_peer = UdpPeer::bind(host_address, guest_address)
            .await
            .expect("host peer");
        let guest_peer = UdpPeer::bind(guest_address, host_address)
            .await
            .expect("guest peer");
        let host_config = MatchProbeConfig::new(
            direct_descriptor(PeerRole::Host, host_address, guest_address),
            "shared-session",
            60,
            Duration::from_secs(2),
        )
        .expect("host config");
        let guest_config = MatchProbeConfig::new(
            direct_descriptor(PeerRole::Guest, guest_address, host_address),
            "shared-session",
            60,
            Duration::from_secs(2),
        )
        .expect("guest config");

        let (host, guest) = tokio::join!(
            run_match_probe(&host_peer, &host_config),
            run_match_probe(&guest_peer, &guest_config)
        );
        let host = host.expect("host report");
        let guest = guest.expect("guest report");
        assert_eq!(host.frames_received, 60);
        assert_eq!(guest.frames_received, 60);
        assert_eq!(host.transcript_checksum, guest.transcript_checksum);
    }

    #[test]
    fn match_probe_rejects_an_in_memory_descriptor() {
        let error = MatchProbeConfig::new(
            descriptor(PeerRole::Host),
            "shared-session",
            60,
            Duration::from_secs(2),
        )
        .expect_err("in-memory descriptor must be rejected");
        assert!(matches!(error, TransportError::InvalidConfiguration(_)));
    }

    fn direct_descriptor(
        role: PeerRole,
        local_endpoint: SocketAddr,
        peer_endpoint: SocketAddr,
    ) -> MatchDescriptor {
        let (local_user_id, peer_user_id) = match role {
            PeerRole::Host => ("host", "guest"),
            PeerRole::Guest => ("guest", "host"),
        };
        MatchDescriptor {
            room_id: "proof-room".into(),
            game_id: "sfiii3".into(),
            local_user_id: local_user_id.into(),
            peer_user_id: peer_user_id.into(),
            role,
            transport: TransportKind::DirectUdp,
            local_endpoint,
            peer_endpoint,
            input_delay_frames: 2,
        }
    }
}
