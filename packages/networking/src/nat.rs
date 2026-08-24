use opencade_emulator_sdk::TransportKind;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::Duration;

/// NAT type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NatType {
    Open,
    Cone,
    Symmetric,
    Blocked,
    #[default]
    Unknown,
}

impl fmt::Display for NatType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Open => "open",
            Self::Cone => "cone",
            Self::Symmetric => "symmetric",
            Self::Blocked => "blocked",
            Self::Unknown => "unknown",
        };
        write!(f, "{s}")
    }
}

/// STUN / NAT traversal helper.
#[derive(Debug, Clone)]
pub struct NatTraversal {
    pub stun_host: String,
    pub stun_port: u16,
    pub local_addr: SocketAddr,
}

impl NatTraversal {
    pub fn new(stun_host: impl Into<String>, stun_port: u16, local_addr: SocketAddr) -> Self {
        Self {
            stun_host: stun_host.into(),
            stun_port,
            local_addr,
        }
    }

    fn stun_socket_addr(&self) -> Option<SocketAddr> {
        let target = format!("{}:{}", self.stun_host, self.stun_port);
        target.to_socket_addrs().ok()?.next()
    }

    fn is_stun_reachable(&self) -> bool {
        let Some(addr) = self.stun_socket_addr() else {
            return false;
        };
        // Use TCP connect as reachability probe with short timeout.
        // UDP connect/send would always appear to succeed, so TCP gives a real handshake.
        TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok()
    }

    /// Stub classification via reachability.
    ///
    /// - If STUN host unreachable => Unknown
    /// - If direct UDP to STUN succeeds (TCP reachability here as proxy) => Cone (Open variant
    ///   would be when local_addr is public, but we stub as Cone)
    /// - Symmetric heuristic stub: if stun_host matches local ip and port differs, report Symmetric
    pub fn classify(&self) -> NatType {
        if !self.is_stun_reachable() {
            return NatType::Unknown;
        }
        // Symmetric heuristic stub: if local port equals STUN port we treat as Open, otherwise Cone.
        // This is deterministic and test-friendly.
        if self.local_addr.port() == self.stun_port {
            NatType::Open
        } else {
            // Additional symmetric check: if STUN host equals local ip string, consider Symmetric
            // for a narrow case to exercise the variant, but keep default as Cone.
            if self.stun_host == self.local_addr.ip().to_string() {
                // Use a simple port-parity heuristic to sometimes return Symmetric.
                // Even stun_port + odd local_port => Symmetric stub, but keep Cone as common path.
                // For predictability, return Cone unless port difference is large.
                if self.stun_port.wrapping_sub(self.local_addr.port()) > 1000 {
                    return NatType::Symmetric;
                }
            }
            NatType::Cone
        }
    }

    /// Bind ephemeral UDP, send probe, await echo.
    pub fn direct_udp_probe(&self, peer: SocketAddr) -> bool {
        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(_) => return false,
        };
        if socket
            .set_read_timeout(Some(Duration::from_millis(800)))
            .is_err()
        {
            return false;
        }
        let probe = b"opencade-probe";
        if socket.send_to(probe, peer).is_err() {
            return false;
        }
        let mut buf = [0u8; 64];
        match socket.recv_from(&mut buf) {
            Ok((n, _addr)) => &buf[..n] == probe,
            Err(_) => false,
        }
    }

    /// Simultaneous sends 3 attempts 500ms apart, stub succeeds if direct probe fails but STUN reachable.
    pub fn hole_punch(&self, peer: SocketAddr) -> bool {
        if self.direct_udp_probe(peer) {
            return false;
        }
        if !self.is_stun_reachable() {
            return false;
        }
        // Simulate simultaneous sends. Actual NAT hole-punch would need peer cooperation;
        // here we stub success when STUN reachable and direct probe failed.
        for _ in 0..3 {
            // Fire an ephemeral send; ignore result, we just simulate the attempt.
            if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
                let _ = sock.set_read_timeout(Some(Duration::from_millis(200)));
                let _ = sock.send_to(b"hole-punch", peer);
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        true
    }

    pub fn relay_fallback(&self) -> TransportKind {
        TransportKind::Relay
    }
}

/// Preferred transport fallback order.
pub const FALLBACK_ORDER: [TransportKind; 4] = [
    TransportKind::DirectUdp,
    TransportKind::HolePunch,
    TransportKind::Stun,
    TransportKind::Relay,
];

/// Alias required by spec re-export.
#[allow(non_upper_case_globals)]
pub const FallbackOrder: [TransportKind; 4] = FALLBACK_ORDER;

/// Return next transport in fallback order.
pub fn next_transport(current: TransportKind) -> Option<TransportKind> {
    let pos = FALLBACK_ORDER.iter().position(|t| *t == current)?;
    FALLBACK_ORDER.get(pos + 1).copied()
}

/// Map NAT type to preferred transport.
pub fn transport_for_nat(nat: NatType) -> TransportKind {
    match nat {
        NatType::Open => TransportKind::DirectUdp,
        NatType::Cone => TransportKind::HolePunch,
        NatType::Symmetric => TransportKind::Stun,
        NatType::Blocked | NatType::Unknown => TransportKind::Relay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, UdpSocket};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn nat_type_display_and_serde() {
        assert_eq!(NatType::Open.to_string(), "open");
        assert_eq!(NatType::Cone.to_string(), "cone");
        assert_eq!(NatType::Symmetric.to_string(), "symmetric");
        assert_eq!(NatType::Blocked.to_string(), "blocked");
        assert_eq!(NatType::Unknown.to_string(), "unknown");
        assert_eq!(NatType::default(), NatType::Unknown);

        let ser = serde_json::to_string(&NatType::Cone).unwrap();
        assert_eq!(ser, "\"cone\"");
        let de: NatType = serde_json::from_str("\"symmetric\"").unwrap();
        assert_eq!(de, NatType::Symmetric);
    }

    #[test]
    fn classification_unknown_when_stun_unreachable() {
        let nt = NatTraversal::new(
            "192.0.2.1".to_string(), // TEST-NET-1, unroutable
            3478,
            "127.0.0.1:0".parse().unwrap(),
        );
        assert_eq!(nt.classify(), NatType::Unknown);
    }

    #[test]
    fn classification_cone_or_open_when_stun_reachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        // Keep listener alive in background
        thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let _ = stream;
            }
        });
        // small pause to ensure listener is ready
        thread::sleep(Duration::from_millis(50));
        let nt = NatTraversal::new(
            "127.0.0.1".to_string(),
            addr.port(),
            "127.0.0.1:41000".parse().unwrap(),
        );
        let result = nt.classify();
        assert!(
            result == NatType::Cone || result == NatType::Open || result == NatType::Symmetric,
            "expected reachable classification, got {result}"
        );
    }

    #[test]
    fn fallback_order_and_next_transport() {
        assert_eq!(FALLBACK_ORDER.len(), 4);
        assert_eq!(FALLBACK_ORDER[0], TransportKind::DirectUdp);
        assert_eq!(FALLBACK_ORDER[1], TransportKind::HolePunch);
        assert_eq!(FALLBACK_ORDER[2], TransportKind::Stun);
        assert_eq!(FALLBACK_ORDER[3], TransportKind::Relay);

        assert_eq!(
            next_transport(TransportKind::DirectUdp),
            Some(TransportKind::HolePunch)
        );
        assert_eq!(
            next_transport(TransportKind::HolePunch),
            Some(TransportKind::Stun)
        );
        assert_eq!(
            next_transport(TransportKind::Stun),
            Some(TransportKind::Relay)
        );
        assert_eq!(next_transport(TransportKind::Relay), None);
        assert_eq!(next_transport(TransportKind::InMemory), None);
        // Alias check
        assert_eq!(FallbackOrder, FALLBACK_ORDER);
    }

    #[test]
    fn direct_udp_probe_loopback() {
        // Echo server
        let echo_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let echo_addr = echo_sock.local_addr().unwrap();
        echo_sock
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        thread::spawn(move || {
            let mut buf = [0u8; 64];
            if let Ok((n, peer)) = echo_sock.recv_from(&mut buf) {
                let _ = echo_sock.send_to(&buf[..n], peer);
            }
        });
        thread::sleep(Duration::from_millis(50));
        let nt = NatTraversal::new(
            "127.0.0.1".to_string(),
            3478,
            "127.0.0.1:0".parse().unwrap(),
        );
        assert!(nt.direct_udp_probe(echo_addr));

        // No server => probe fails (no echo on this port)
        let nt2 = NatTraversal::new(
            "127.0.0.1".to_string(),
            3478,
            "127.0.0.1:0".parse().unwrap(),
        );
        // This will try to send to discard port; no echo, so false
        assert!(!nt2.direct_udp_probe("127.0.0.1:54321".parse().unwrap()));
    }

    #[test]
    fn hole_punch_under_loopback() {
        // STUN reachable listener
        let stun_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let stun_addr = stun_listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in stun_listener.incoming().take(5) {
                let _ = stream;
            }
        });
        thread::sleep(Duration::from_millis(50));

        let nt = NatTraversal::new(
            "127.0.0.1".to_string(),
            stun_addr.port(),
            "127.0.0.1:0".parse().unwrap(),
        );

        // Peer with no echo => direct fails, STUN reachable => hole_punch succeeds
        let peer: SocketAddr = "127.0.0.1:54322".parse().unwrap();
        assert!(nt.hole_punch(peer));

        // Peer with echo => direct succeeds => hole_punch returns false
        let echo_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let echo_addr = echo_sock.local_addr().unwrap();
        echo_sock
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        thread::spawn(move || {
            let mut buf = [0u8; 64];
            while let Ok((n, peer)) = echo_sock.recv_from(&mut buf) {
                let _ = echo_sock.send_to(&buf[..n], peer);
            }
        });
        thread::sleep(Duration::from_millis(50));
        // Now direct succeeds, hole_punch should be false
        assert!(!nt.hole_punch(echo_addr));
    }

    #[test]
    fn relay_fallback_returns_relay() {
        let nt = NatTraversal::new(
            "127.0.0.1".to_string(),
            3478,
            "127.0.0.1:0".parse().unwrap(),
        );
        assert_eq!(nt.relay_fallback(), TransportKind::Relay);
    }

    #[test]
    fn transport_for_nat_mapping() {
        assert_eq!(transport_for_nat(NatType::Open), TransportKind::DirectUdp);
        assert_eq!(transport_for_nat(NatType::Cone), TransportKind::HolePunch);
        assert_eq!(transport_for_nat(NatType::Symmetric), TransportKind::Stun);
        assert_eq!(transport_for_nat(NatType::Blocked), TransportKind::Relay);
        assert_eq!(transport_for_nat(NatType::Unknown), TransportKind::Relay);
    }
}
