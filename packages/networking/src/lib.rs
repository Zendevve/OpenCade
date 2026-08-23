use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::sync::mpsc;

pub const MAX_INPUT_BYTES: usize = 256;
pub const INPUT_QUEUE_CAPACITY: usize = 120;

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
}

/// Connected UDP transport for LAN proof runs. Authentication and endpoint negotiation remain in
/// the control plane; this type only carries bounded OpenFight input frames.
pub struct UdpPeer {
    socket: tokio::net::UdpSocket,
}

impl UdpPeer {
    pub async fn bind(local: SocketAddr, peer: SocketAddr) -> Result<Self, TransportError> {
        let socket = tokio::net::UdpSocket::bind(local)
            .await
            .map_err(|error| TransportError::Io(error.to_string()))?;
        socket
            .connect(peer)
            .await
            .map_err(|error| TransportError::Io(error.to_string()))?;
        Ok(Self { socket })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        self.socket
            .local_addr()
            .map_err(|error| TransportError::Io(error.to_string()))
    }

    pub async fn send(&self, frame: &InputFrame) -> Result<(), TransportError> {
        if frame.input.len() > MAX_INPUT_BYTES || frame.player_id.trim().is_empty() {
            return Err(TransportError::Serialization("invalid input frame".into()));
        }
        let encoded = serde_json::to_vec(frame)
            .map_err(|error| TransportError::Serialization(error.to_string()))?;
        self.socket
            .send(&encoded)
            .await
            .map_err(|error| TransportError::Io(error.to_string()))?;
        Ok(())
    }

    pub async fn receive(&self) -> Result<InputFrame, TransportError> {
        let mut buffer = [0_u8; 1024];
        let received = self
            .socket
            .recv(&mut buffer)
            .await
            .map_err(|error| TransportError::Io(error.to_string()))?;
        let frame: InputFrame = serde_json::from_slice(&buffer[..received])
            .map_err(|error| TransportError::Serialization(error.to_string()))?;
        InputFrame::new(frame.frame, frame.player_id, frame.input)
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
    use openfight_emulator_sdk::{
        EmulatorAdapter, MatchDescriptor, MockAdapter, PeerRole, TransportKind,
    };

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
}
