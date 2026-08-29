//! OpenCade proof-of-match — drives a deterministic two-peer `MockAdapter` scenario over
//! `InMemoryPeer` and emits canonical redacted `MatchReport` evidence that
//! `opencade_networking::verify_match_reports` accepts.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use chrono::Utc;
use opencade_emulator_sdk::{
    EmulatorAdapter, MatchDescriptor, MockAdapter, PeerRole, TransportKind,
};
use opencade_networking::{InMemoryPeer, InputFrame, MatchVerification, verify_match_reports};
use opencade_protocol::{
    MatchCandidateKind, MatchReport, MatchReportClient, MatchReportProbe, MatchReportRole,
    MatchReportRoom, MatchReportTransport, NatMappingState, RoomState,
};
use thiserror::Error;

/// Number of frames exchanged in a proof-of-match run. Matches the alpha LAN evidence contract
/// in `opencade_networking::ALPHA_MATCH_FRAMES`.
pub const PROOF_FRAMES: u64 = 60;

/// FNV-1a 64-bit constants. Mirrored from `packages/networking/src/probe.rs:281-282` because
/// the original constants are private to `opencade_networking::probe`. Any change here MUST
/// also be reflected in the LAN alpha probe; the pair verifier compares the resulting checksums
/// byte-for-byte.
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

const RETRY_INTERVAL: Duration = Duration::from_millis(25);
const PROOF_DEADLINE: Duration = Duration::from_secs(5);

/// Input to a single proof-of-match run.
#[derive(Debug, Clone)]
pub struct ProofConfig {
    pub room_id: String,
    pub game_id: String,
    pub host_user_id: String,
    pub guest_user_id: String,
    pub host_endpoint: SocketAddr,
    pub guest_endpoint: SocketAddr,
    pub input_delay_frames: u8,
    pub platform: String,
}

/// Output of a successful proof-of-match run: the two redacted reports and the verifier's
/// accept record.
#[derive(Debug, Clone)]
pub struct ProofReport {
    pub host: MatchReport,
    pub guest: MatchReport,
    pub verification: MatchVerification,
}

/// Errors produced by the proof-of-match runner. All variants carry a stable short code so the
/// bin and the integration tests can assert on the failure category.
#[derive(Debug, Error)]
pub enum ProofError {
    #[error("match preparation failed: {0}")]
    Preparation(String),
    #[error("transport failed: {0}")]
    Transport(String),
    #[error("invalid input frame: {0}")]
    Frame(String),
    #[error("verification rejected the evidence: {code}: {message}")]
    Verification { code: &'static str, message: String },
}

impl ProofError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Preparation(_) => "preparation_failed",
            Self::Transport(_) => "transport_failed",
            Self::Frame(_) => "frame_invalid",
            Self::Verification { code, .. } => code,
        }
    }
}

/// Run a deterministic two-peer proof-of-match scenario.
///
/// The function is async and does not own a runtime; callers may drive it from `#[tokio::test]`,
/// a CLI bin with a manually-constructed `tokio::runtime::Runtime`, or any other executor that
/// supports the `macros` + `rt` features of `tokio` already enabled by `opencade_networking`.
///
/// Behaviour:
/// 1. Build the host and guest `MatchDescriptor`s with `TransportKind::DirectUdp` (the evidence
///    label, not the wire choice) and call `MockAdapter::prepare_match` on each.
/// 2. Pair an `InMemoryPeer` and drive 60 deterministic input frames in each direction,
///    mirroring the retry policy of `opencade_networking::run_match_probe`.
/// 3. Build a canonical redacted `MatchReport` per side and call
///    `opencade_networking::verify_match_reports` to confirm the transcript is identical.
pub async fn run_proof(config: &ProofConfig) -> Result<ProofReport, ProofError> {
    let host_desc = MatchDescriptor {
        room_id: config.room_id.clone(),
        game_id: config.game_id.clone(),
        local_user_id: config.host_user_id.clone(),
        peer_user_id: config.guest_user_id.clone(),
        role: PeerRole::Host,
        transport: TransportKind::DirectUdp,
        local_endpoint: config.host_endpoint,
        peer_endpoint: config.guest_endpoint,
        input_delay_frames: config.input_delay_frames,
    };
    let guest_desc = MatchDescriptor {
        room_id: config.room_id.clone(),
        game_id: config.game_id.clone(),
        local_user_id: config.guest_user_id.clone(),
        peer_user_id: config.host_user_id.clone(),
        role: PeerRole::Guest,
        transport: TransportKind::DirectUdp,
        local_endpoint: config.guest_endpoint,
        peer_endpoint: config.host_endpoint,
        input_delay_frames: config.input_delay_frames,
    };

    let adapter = MockAdapter::default();
    adapter
        .prepare_match(&host_desc)
        .map_err(|e| ProofError::Preparation(e.to_string()))?;
    adapter
        .prepare_match(&guest_desc)
        .map_err(|e| ProofError::Preparation(e.to_string()))?;

    let (mut host_peer, mut guest_peer) = InMemoryPeer::pair();

    let host_started = Instant::now();
    let guest_started = Instant::now();
    let (host_outcome, guest_outcome) = tokio::join!(
        drive_side(
            &mut host_peer,
            PeerRole::Host,
            &config.host_user_id,
            &config.guest_user_id,
        ),
        drive_side(
            &mut guest_peer,
            PeerRole::Guest,
            &config.guest_user_id,
            &config.host_user_id,
        ),
    );
    drop(host_peer);
    drop(guest_peer);
    let host_outcome = host_outcome?;
    let guest_outcome = guest_outcome?;

    let user_agent = format!("opencade-proof-of-match/{}", env!("CARGO_PKG_VERSION"));

    let host_report = build_report(
        &config.room_id,
        &config.game_id,
        &config.platform,
        &user_agent,
        PeerRole::Host,
        host_outcome,
        host_started.elapsed().as_millis() as u32,
    );
    let guest_report = build_report(
        &config.room_id,
        &config.game_id,
        &config.platform,
        &user_agent,
        PeerRole::Guest,
        guest_outcome,
        guest_started.elapsed().as_millis() as u32,
    );
    let verification = verify_match_reports(&host_report, &guest_report).map_err(|error| {
        // `verify_match_reports` is the canonical authority on error codes; we propagate
        // its stable `code()` directly so the bin and the integration test can assert on it.
        let code = error.code();
        ProofError::Verification {
            code,
            message: format!("{error}"),
        }
    })?;

    Ok(ProofReport {
        host: host_report,
        guest: guest_report,
        verification,
    })
}

struct DriveOutcome {
    frames_sent: u32,
    frames_received: u32,
    checksum: u64,
}

async fn drive_side(
    peer: &mut InMemoryPeer,
    role: PeerRole,
    local_user_id: &str,
    peer_user_id: &str,
) -> Result<DriveOutcome, ProofError> {
    let deadline = Instant::now() + PROOF_DEADLINE;
    let mut frames_sent: u32 = 0;
    let mut frames_received: u32 = 0;
    let mut checksum = FNV_OFFSET_BASIS;

    for frame_number in 0..PROOF_FRAMES {
        let local = deterministic_frame(frame_number, local_user_id, role)
            .map_err(|e| ProofError::Frame(e.to_string()))?;

        // Send the local frame. Backpressure or a closed peer surfaces immediately.
        peer.try_send(local.clone())
            .map_err(|e| ProofError::Transport(e.to_string()))?;
        frames_sent += 1;

        // Await the matching remote frame. Out-of-order or older remote frames trigger a
        // retransmit of the local frame, matching `run_match_probe`'s policy.
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(ProofError::Transport(format!(
                    "timed out after receiving {frames_received} of {PROOF_FRAMES} frames"
                )));
            }
            let wait = RETRY_INTERVAL.min(deadline.saturating_duration_since(now));
            match tokio::time::timeout(wait, peer.receive()).await {
                Err(_) => {
                    peer.try_send(local.clone())
                        .map_err(|e| ProofError::Transport(e.to_string()))?;
                    frames_sent += 1;
                }
                Ok(Err(e)) => return Err(ProofError::Transport(e.to_string())),
                Ok(Ok(remote)) => {
                    if remote.frame < frame_number {
                        let previous = deterministic_frame(remote.frame, local_user_id, role)
                            .map_err(|e| ProofError::Frame(e.to_string()))?;
                        peer.try_send(previous)
                            .map_err(|e| ProofError::Transport(e.to_string()))?;
                        frames_sent += 1;
                        continue;
                    }
                    if remote.frame > frame_number {
                        peer.try_send(local.clone())
                            .map_err(|e| ProofError::Transport(e.to_string()))?;
                        frames_sent += 1;
                        continue;
                    }
                    update_transcript_checksum(role, &local, &remote, &mut checksum);
                    frames_received += 1;
                    break;
                }
            }
        }

        // `peer_user_id` is referenced only by the descriptor; the actual data plane does not
        // attach it. Keep the variable live to document the contract.
        let _ = peer_user_id;
    }

    Ok(DriveOutcome {
        frames_sent,
        frames_received,
        checksum,
    })
}

fn deterministic_frame(
    frame: u64,
    player_id: &str,
    role: PeerRole,
) -> Result<InputFrame, opencade_networking::TransportError> {
    let input = match role {
        PeerRole::Host => (frame % 4) as u8,
        PeerRole::Guest => ((frame + 2) % 4) as u8,
    };
    InputFrame::new(frame, player_id, vec![input])
}

fn update_transcript_checksum(
    role: PeerRole,
    local: &InputFrame,
    remote: &InputFrame,
    checksum: &mut u64,
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

fn build_report(
    room_id: &str,
    game_id: &str,
    platform: &str,
    user_agent: &str,
    role: PeerRole,
    outcome: DriveOutcome,
    elapsed_ms: u32,
) -> MatchReport {
    let transcript_checksum = format!("{:016x}", outcome.checksum);
    let report_role = match role {
        PeerRole::Host => MatchReportRole::Host,
        PeerRole::Guest => MatchReportRole::Guest,
    };
    MatchReport {
        schema_version: 1,
        exported_at: Utc::now(),
        room: MatchReportRoom {
            id: room_id.to_string(),
            game_id: game_id.to_string(),
            state: RoomState::Finished,
        },
        probe: MatchReportProbe {
            role: report_role,
            transport: MatchReportTransport::DirectUdp,
            frames_sent: outcome.frames_sent,
            frames_received: outcome.frames_received,
            transcript_checksum,
            elapsed_ms,
            nat: Some(NatMappingState::Unknown),
            candidate: Some(MatchCandidateKind::Host),
            punch_attempts: Some(0),
        },
        client: MatchReportClient {
            platform: platform.to_string(),
            user_agent: user_agent.to_string(),
        },
        // The keystone uses `MockAdapter` and does not launch a real native emulator, so
        // there is no native emulator compatibility fingerprint to attest. The pair verifier
        // requires the two reports to agree, and `None == None` is true. Real emulator runs
        // (FBNeo / RetroArch) go through `verify_playable_match_reports` instead, which
        // requires `Some(_)` on both sides.
        compatibility: None,
        native_route: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_config() -> ProofConfig {
        ProofConfig {
            room_id: "demo".into(),
            game_id: "sfiii3".into(),
            host_user_id: "alice".into(),
            guest_user_id: "bob".into(),
            host_endpoint: "127.0.0.1:41000".parse().unwrap(),
            guest_endpoint: "127.0.0.1:41001".parse().unwrap(),
            input_delay_frames: 0,
            platform: "proof".into(),
        }
    }

    #[test]
    fn deterministic_frame_matches_probe_contract() {
        let host = deterministic_frame(0, "alice", PeerRole::Host).unwrap();
        assert_eq!(host.input, vec![0]);
        let host = deterministic_frame(3, "alice", PeerRole::Host).unwrap();
        assert_eq!(host.input, vec![3]);
        let guest = deterministic_frame(0, "bob", PeerRole::Guest).unwrap();
        assert_eq!(guest.input, vec![2]);
        let guest = deterministic_frame(3, "bob", PeerRole::Guest).unwrap();
        assert_eq!(guest.input, vec![1]);
    }

    #[test]
    fn fnv_checksum_known_value() {
        let local = InputFrame::new(0, "alice", vec![0]).unwrap();
        let remote = InputFrame::new(0, "bob", vec![2]).unwrap();
        let mut checksum = FNV_OFFSET_BASIS;
        update_transcript_checksum(PeerRole::Host, &local, &remote, &mut checksum);
        // For Host role the byte order is host=[0] then guest=[2]: 0 -> 0, 2 -> 2*prime.
        let expected = {
            let mut c = FNV_OFFSET_BASIS;
            c ^= 0;
            c = c.wrapping_mul(FNV_PRIME);
            c ^= 2;
            c.wrapping_mul(FNV_PRIME)
        };
        assert_eq!(checksum, expected);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_proof_happy_path() {
        let report = run_proof(&canonical_config())
            .await
            .expect("proof succeeds");
        assert!(report.verification.verified);
        assert_eq!(report.host.probe.frames_received, PROOF_FRAMES as u32);
        assert_eq!(report.guest.probe.frames_received, PROOF_FRAMES as u32);
        assert_eq!(
            report.host.probe.transcript_checksum,
            report.guest.probe.transcript_checksum,
        );
        assert_ne!(report.host.probe.role, report.guest.probe.role);
        assert_eq!(report.host.room.state, RoomState::Finished);
        assert_eq!(report.guest.room.state, RoomState::Finished);
        assert_eq!(report.host.schema_version, 1);
        assert_eq!(report.guest.schema_version, 1);
    }
}
