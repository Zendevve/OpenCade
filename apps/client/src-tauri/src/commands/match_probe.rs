use opencade_emulator_sdk::{MatchDescriptor, PeerRole, TransportKind};
use opencade_networking::{
    HolePunchConfig, MatchProbeConfig, MatchProbeReport, RelayPeer, UdpPeer,
    discover_reflexive_address, punch_hole, run_match_probe, run_relay_match_probe,
};
use opencade_protocol::{MatchCandidateKind, NatMappingState};
use opencade_shared::RelayTicket;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::sync::{Mutex, watch};
use uuid::Uuid;

const MAX_ACTIVE_PROBES: usize = 8;

#[derive(Default)]
pub struct MatchProbeState {
    probes: Mutex<HashMap<String, MatchProbeSlot>>,
}

enum MatchProbeSlot {
    Prepared(PreparedProbe),
    Running {
        run_id: Uuid,
        cancel: watch::Sender<bool>,
    },
}

struct PreparedProbe {
    peer: UdpPeer,
    candidate: MatchEndpointCandidate,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchEndpointCandidate {
    pub endpoint: SocketAddr,
    pub reflexive_endpoint: Option<SocketAddr>,
    pub nat: NatMappingState,
    pub nonce: String,
}

#[derive(Debug, Deserialize)]
pub struct ReserveMatchProbeRequest {
    pub room_id: String,
    pub advertised_host: Option<Ipv4Addr>,
    pub bind_port: Option<u16>,
    pub stun_server: Option<SocketAddr>,
}

#[derive(Debug, Deserialize)]
pub struct RunMatchProbeRequest {
    pub room_id: String,
    pub game_id: String,
    pub local_user_id: String,
    pub peer_user_id: String,
    pub role: PeerRole,
    pub peer_endpoint: SocketAddr,
    pub peer_reflexive_endpoint: Option<SocketAddr>,
    pub peer_nonce: String,
    #[serde(default = "default_frame_count")]
    pub frame_count: u64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct RunRelayMatchProbeRequest {
    pub relay_url: String,
    pub ticket: RelayTicket,
    pub room_id: String,
    pub game_id: String,
    pub local_user_id: String,
    pub peer_user_id: String,
    pub role: PeerRole,
    pub local_nonce: String,
    pub peer_nonce: String,
    #[serde(default = "default_frame_count")]
    pub frame_count: u64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_frame_count() -> u64 {
    60
}

fn default_timeout_ms() -> u64 {
    5_000
}

#[tauri::command]
pub async fn reserve_match_probe(
    state: tauri::State<'_, MatchProbeState>,
    request: ReserveMatchProbeRequest,
) -> Result<MatchEndpointCandidate, String> {
    validate_room_id(&request.room_id)?;
    if let Some(slot) = state.probes.lock().await.get(&request.room_id) {
        return match slot {
            MatchProbeSlot::Prepared(prepared) => Ok(prepared.candidate.clone()),
            MatchProbeSlot::Running { .. } => {
                Err("match probe is already running for this room".into())
            }
        };
    }

    let bind_address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, request.bind_port.unwrap_or(0)));
    let peer = UdpPeer::bind_unconnected(bind_address)
        .await
        .map_err(|error| error.to_string())?;
    let port = peer.local_addr().map_err(|error| error.to_string())?.port();
    let host = match request.advertised_host {
        Some(host) if !host.is_unspecified() => host,
        Some(_) => return Err("advertised_host must not be unspecified".into()),
        None => discover_local_ipv4()?,
    };
    let endpoint = SocketAddr::from((host, port));
    let stun = match request.stun_server {
        Some(server) => {
            match discover_reflexive_address(&peer, server, Duration::from_millis(1_500)).await {
                Ok(observation) => Some(observation.reflexive_endpoint),
                Err(error) => {
                    tracing::warn!(%error, %server, "STUN discovery failed; retaining host candidate");
                    None
                }
            }
        }
        None => None,
    };
    let candidate = MatchEndpointCandidate {
        endpoint,
        reflexive_endpoint: stun.filter(|reflexive| *reflexive != endpoint),
        nat: match stun {
            Some(reflexive) if reflexive == endpoint => NatMappingState::Open,
            Some(_) => NatMappingState::Mapped,
            None => NatMappingState::Unknown,
        },
        nonce: Uuid::new_v4().to_string(),
    };
    let mut probes = state.probes.lock().await;
    if let Some(existing) = probes.get(&request.room_id) {
        return match existing {
            MatchProbeSlot::Prepared(prepared) => Ok(prepared.candidate.clone()),
            MatchProbeSlot::Running { .. } => {
                Err("match probe is already running for this room".into())
            }
        };
    }
    if probes.len() >= MAX_ACTIVE_PROBES {
        return Err("too many active match probes; cancel an older room first".into());
    }
    probes.insert(
        request.room_id,
        MatchProbeSlot::Prepared(PreparedProbe {
            peer,
            candidate: candidate.clone(),
        }),
    );
    Ok(candidate)
}

#[tauri::command]
pub async fn run_reserved_match_probe(
    state: tauri::State<'_, MatchProbeState>,
    request: RunMatchProbeRequest,
) -> Result<MatchProbeReport, String> {
    validate_room_id(&request.room_id)?;
    let room_id = request.room_id.clone();
    let run_id = Uuid::new_v4();
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let prepared = {
        let mut probes = state.probes.lock().await;
        let slot = probes
            .remove(&request.room_id)
            .ok_or_else(|| "match probe is not reserved for this room".to_string())?;
        let MatchProbeSlot::Prepared(prepared) = slot else {
            probes.insert(request.room_id.clone(), slot);
            return Err("match probe is already running for this room".into());
        };
        probes.insert(
            request.room_id.clone(),
            MatchProbeSlot::Running {
                run_id,
                cancel: cancel_tx,
            },
        );
        prepared
    };
    let probe = async move {
        let session_key = combined_session_key(&prepared.candidate.nonce, &request.peer_nonce)?;
        let mut peer_candidates = vec![request.peer_endpoint];
        if let Some(reflexive) = request.peer_reflexive_endpoint {
            peer_candidates.push(reflexive);
        }
        let punch_config = HolePunchConfig::new(
            request.room_id.clone(),
            session_key.clone(),
            peer_candidates,
            3,
            Duration::from_millis(500),
        )
        .map_err(|error| error.to_string())?;
        let (peer, punch) = punch_hole(prepared.peer, &punch_config)
            .await
            .map_err(|error| error.to_string())?;
        let candidate_kind = if request.peer_reflexive_endpoint == Some(punch.selected_peer)
            && request.peer_reflexive_endpoint != Some(request.peer_endpoint)
        {
            MatchCandidateKind::Reflexive
        } else {
            MatchCandidateKind::Host
        };
        let descriptor = MatchDescriptor {
            room_id: request.room_id,
            game_id: request.game_id,
            local_user_id: request.local_user_id,
            peer_user_id: request.peer_user_id,
            role: request.role,
            transport: TransportKind::DirectUdp,
            local_endpoint: prepared.candidate.endpoint,
            peer_endpoint: punch.selected_peer,
            input_delay_frames: 2,
        };
        let config = MatchProbeConfig::new(
            descriptor,
            session_key,
            request.frame_count,
            Duration::from_millis(request.timeout_ms),
        )
        .map_err(|error| error.to_string())?;
        let mut report = run_match_probe(&peer, &config)
            .await
            .map_err(|error| error.to_string())?;
        report.nat = prepared.candidate.nat;
        report.candidate = candidate_kind;
        report.punch_attempts = punch.attempts;
        Ok(report)
    };
    let result = cancel_or_complete(probe, cancel_rx).await;
    remove_running_probe(&state, &room_id, run_id).await;
    let report = result?;
    tracing::info!(
        room_id = %report.room_id,
        frames = report.frames_received,
        elapsed_ms = report.elapsed_ms,
        "LAN match probe completed"
    );
    Ok(report)
}

#[tauri::command]
pub async fn run_relay_match_probe_command(
    request: RunRelayMatchProbeRequest,
) -> Result<MatchProbeReport, String> {
    validate_room_id(&request.room_id)?;
    if request.ticket.room_id != request.room_id || request.ticket.user_id != request.local_user_id
    {
        return Err("relay ticket does not match the local room participant".into());
    }
    let session_key = combined_session_key(&request.local_nonce, &request.peer_nonce)?;
    let descriptor = MatchDescriptor {
        room_id: request.room_id,
        game_id: request.game_id,
        local_user_id: request.local_user_id,
        peer_user_id: request.peer_user_id,
        role: request.role,
        transport: TransportKind::Relay,
        local_endpoint: "0.0.0.0:0".parse().map_err(|_| "invalid relay endpoint")?,
        peer_endpoint: "0.0.0.0:0".parse().map_err(|_| "invalid relay endpoint")?,
        input_delay_frames: 2,
    };
    let config = MatchProbeConfig::new(
        descriptor,
        session_key,
        request.frame_count,
        Duration::from_millis(request.timeout_ms),
    )
    .map_err(|error| error.to_string())?;
    let peer = RelayPeer::connect(&request.relay_url, &request.ticket)
        .await
        .map_err(|error| error.to_string())?;
    let report = run_relay_match_probe(&peer, &config)
        .await
        .map_err(|error| error.to_string())?;
    tracing::info!(
        room_id = %report.room_id,
        frames = report.frames_received,
        elapsed_ms = report.elapsed_ms,
        "relay match probe completed"
    );
    Ok(report)
}

#[tauri::command]
pub async fn cancel_match_probe(
    state: tauri::State<'_, MatchProbeState>,
    room_id: String,
) -> Result<(), String> {
    validate_room_id(&room_id)?;
    cancel_probe(&state, &room_id).await;
    Ok(())
}

async fn cancel_probe(state: &MatchProbeState, room_id: &str) {
    if let Some(MatchProbeSlot::Running { cancel, .. }) = state.probes.lock().await.remove(room_id)
    {
        let _ = cancel.send(true);
    }
}

async fn remove_running_probe(state: &MatchProbeState, room_id: &str, run_id: Uuid) {
    let mut probes = state.probes.lock().await;
    if matches!(
        probes.get(room_id),
        Some(MatchProbeSlot::Running {
            run_id: current_id,
            ..
        }) if *current_id == run_id
    ) {
        probes.remove(room_id);
    }
}

async fn cancel_or_complete<F, T>(future: F, mut cancel: watch::Receiver<bool>) -> Result<T, String>
where
    F: Future<Output = Result<T, String>>,
{
    if *cancel.borrow() {
        return Err("match probe cancelled".into());
    }
    tokio::select! {
        result = future => result,
        _ = cancel.changed() => Err("match probe cancelled".into()),
    }
}

fn validate_room_id(room_id: &str) -> Result<(), String> {
    Uuid::parse_str(room_id)
        .map(|_| ())
        .map_err(|_| "room_id must be a UUID".into())
}

fn discover_local_ipv4() -> Result<Ipv4Addr, String> {
    let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|error| format!("failed to inspect local network: {error}"))?;
    socket
        .connect((Ipv4Addr::new(192, 0, 2, 1), 9))
        .map_err(|error| format!("failed to select a LAN interface: {error}"))?;
    match socket
        .local_addr()
        .map_err(|error| format!("failed to read LAN address: {error}"))?
        .ip()
    {
        IpAddr::V4(address) if !address.is_unspecified() => Ok(address),
        _ => Err("no usable IPv4 LAN address was detected".into()),
    }
}

fn combined_session_key(local_nonce: &str, peer_nonce: &str) -> Result<String, String> {
    let local = Uuid::parse_str(local_nonce).map_err(|_| "local nonce is invalid")?;
    let peer = Uuid::parse_str(peer_nonce).map_err(|_| "peer nonce is invalid")?;
    if local == peer {
        return Err("peer nonce must differ from local nonce".into());
    }
    let (first, second) = if local.as_bytes() < peer.as_bytes() {
        (local, peer)
    } else {
        (peer, local)
    };
    Ok(format!("{first}:{second}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_key_is_independent_of_peer_order() {
        let first = "8a1110d5-8dd2-4ad2-9c88-ad9768bc4905";
        let second = "7fa7b981-f41c-4d40-a4c9-d2170067f12f";
        assert_eq!(
            combined_session_key(first, second),
            combined_session_key(second, first)
        );
    }

    #[test]
    fn session_key_rejects_duplicate_nonces() {
        let nonce = "8a1110d5-8dd2-4ad2-9c88-ad9768bc4905";
        assert!(combined_session_key(nonce, nonce).is_err());
    }

    #[tokio::test]
    async fn cancellation_interrupts_pending_probe() {
        let (cancel_tx, cancel_rx) = watch::channel(false);
        cancel_tx.send(true).expect("receiver remains active");
        let result =
            cancel_or_complete(std::future::pending::<Result<(), String>>(), cancel_rx).await;
        assert_eq!(result, Err("match probe cancelled".into()));
    }

    #[tokio::test]
    async fn completed_probe_returns_its_result() {
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let result = cancel_or_complete(async { Ok::<_, String>(42) }, cancel_rx).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn cancelling_running_probe_signals_and_removes_it() {
        let state = MatchProbeState::default();
        let (cancel, cancel_rx) = watch::channel(false);
        state.probes.lock().await.insert(
            "room".into(),
            MatchProbeSlot::Running {
                run_id: Uuid::new_v4(),
                cancel,
            },
        );

        cancel_probe(&state, "room").await;

        assert!(*cancel_rx.borrow());
        assert!(!state.probes.lock().await.contains_key("room"));
    }

    #[tokio::test]
    async fn old_run_cleanup_preserves_newer_run() {
        let state = MatchProbeState::default();
        let old_run_id = Uuid::new_v4();
        let new_run_id = Uuid::new_v4();
        let (cancel, _cancel_rx) = watch::channel(false);
        state.probes.lock().await.insert(
            "room".into(),
            MatchProbeSlot::Running {
                run_id: new_run_id,
                cancel,
            },
        );

        remove_running_probe(&state, "room", old_run_id).await;

        assert!(matches!(
            state.probes.lock().await.get("room"),
            Some(MatchProbeSlot::Running { run_id, .. }) if *run_id == new_run_id
        ));
    }
}
