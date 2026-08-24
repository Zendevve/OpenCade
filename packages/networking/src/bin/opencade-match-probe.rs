use chrono::Utc;
use opencade_emulator_sdk::{MatchDescriptor, PeerRole, TransportKind};
use opencade_networking::{MatchProbeConfig, UdpPeer, run_match_probe};
use opencade_protocol::{
    MATCH_REPORT_SCHEMA_VERSION, MatchReport, MatchReportClient, MatchReportProbe, MatchReportRole,
    MatchReportRoom, MatchReportTransport, RoomState,
};
use std::env;
use std::net::SocketAddr;
use std::time::Duration;

struct Arguments {
    local: SocketAddr,
    peer: SocketAddr,
    room_id: String,
    game_id: String,
    local_user_id: String,
    peer_user_id: String,
    role: PeerRole,
    session_key: String,
    frames: u64,
    timeout: Duration,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut value = |flag: &str| -> Result<String, String> {
            let actual = args
                .next()
                .ok_or_else(|| format!("missing {flag}; {}", usage()))?;
            if actual != flag {
                return Err(format!("expected {flag}, found {actual}; {}", usage()));
            }
            args.next()
                .ok_or_else(|| format!("missing value for {flag}; {}", usage()))
        };

        let local = value("--local")?
            .parse()
            .map_err(|_| "--local must be a socket address".to_string())?;
        let peer = value("--peer")?
            .parse()
            .map_err(|_| "--peer must be a socket address".to_string())?;
        let room_id = value("--room")?;
        let game_id = value("--game")?;
        let local_user_id = value("--local-user")?;
        let peer_user_id = value("--peer-user")?;
        let role = match value("--role")?.as_str() {
            "host" => PeerRole::Host,
            "guest" => PeerRole::Guest,
            _ => return Err("--role must be host or guest".into()),
        };
        let session_key = value("--session-key")?;
        let frames = value("--frames")?
            .parse()
            .map_err(|_| "--frames must be an integer".to_string())?;
        let timeout_ms: u64 = value("--timeout-ms")?
            .parse()
            .map_err(|_| "--timeout-ms must be an integer".to_string())?;
        if let Some(extra) = args.next() {
            return Err(format!("unexpected argument {extra}; {}", usage()));
        }
        Ok(Self {
            local,
            peer,
            room_id,
            game_id,
            local_user_id,
            peer_user_id,
            role,
            session_key,
            frames,
            timeout: Duration::from_millis(timeout_ms),
        })
    }
}

fn usage() -> &'static str {
    "usage: opencade-match-probe --local IP:PORT --peer IP:PORT --room ID --game ID \
     --local-user ID --peer-user ID --role host|guest --session-key KEY --frames N --timeout-ms N"
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse()
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    let room_id = arguments.room_id.clone();
    let game_id = arguments.game_id.clone();
    let descriptor = MatchDescriptor {
        room_id: arguments.room_id,
        game_id: arguments.game_id,
        local_user_id: arguments.local_user_id,
        peer_user_id: arguments.peer_user_id,
        role: arguments.role,
        transport: TransportKind::DirectUdp,
        local_endpoint: arguments.local,
        peer_endpoint: arguments.peer,
        input_delay_frames: 2,
    };
    let config = MatchProbeConfig::new(
        descriptor,
        arguments.session_key,
        arguments.frames,
        arguments.timeout,
    )?;
    let peer = UdpPeer::bind(arguments.local, arguments.peer).await?;
    let probe = run_match_probe(&peer, &config).await?;
    let report = MatchReport {
        schema_version: MATCH_REPORT_SCHEMA_VERSION,
        exported_at: Utc::now(),
        room: MatchReportRoom {
            id: room_id,
            game_id,
            state: RoomState::Finished,
        },
        probe: MatchReportProbe {
            role: match probe.role {
                PeerRole::Host => MatchReportRole::Host,
                PeerRole::Guest => MatchReportRole::Guest,
            },
            transport: MatchReportTransport::DirectUdp,
            frames_sent: u32::try_from(probe.frames_sent).unwrap_or(u32::MAX),
            frames_received: u32::try_from(probe.frames_received).unwrap_or(u32::MAX),
            transcript_checksum: probe.transcript_checksum,
            elapsed_ms: u32::try_from(probe.elapsed_ms).unwrap_or(u32::MAX),
            nat: Some(probe.nat),
            candidate: Some(probe.candidate),
            punch_attempts: Some(probe.punch_attempts),
        },
        client: MatchReportClient {
            platform: std::env::consts::OS.into(),
            user_agent: format!("opencade-match-probe/{}", env!("CARGO_PKG_VERSION")),
        },
        compatibility: None,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
