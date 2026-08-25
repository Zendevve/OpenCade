//! `opencade-proof-of-match` — drive a deterministic two-peer proof-of-match scenario and
//! emit canonical redacted `MatchReport` evidence.
//!
//! The transport used by the proof is the in-process `InMemoryPeer`. The transport label on
//! the produced `MatchReport` is `direct_udp` because the verifier's contract is keyed on
//! the evidence type, not the actual wire choice.

use std::net::SocketAddr;
use std::process::ExitCode;

use opencade_proof_of_match::{ProofConfig, run_proof};

const USAGE: &str = "\
opencade-proof-of-match — deterministic two-peer proof-of-match runner

USAGE:
    opencade-proof-of-match --room <id> --game <id> --host-user <id> --guest-user <id> --platform <str> [--out <path>]

OPTIONS:
    --room <id>         Room identifier (e.g. demo)
    --game <id>         Game identifier (e.g. sfiii3)
    --host-user <id>    Host user identifier
    --guest-user <id>   Guest user identifier
    --platform <str>    Non-sensitive platform label written to the report
    --out <path>        Write JSON evidence to <path> instead of stdout
    --help              Print this help
";

#[derive(Debug, Default)]
struct Args {
    room: Option<String>,
    game: Option<String>,
    host_user: Option<String>,
    guest_user: Option<String>,
    platform: Option<String>,
    out: Option<String>,
    help: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut iter = std::env::args().skip(1);
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--room" => args.room = iter.next(),
            "--game" => args.game = iter.next(),
            "--host-user" => args.host_user = iter.next(),
            "--guest-user" => args.guest_user = iter.next(),
            "--platform" => args.platform = iter.next(),
            "--out" => args.out = iter.next(),
            "--help" | "-h" => args.help = true,
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    Ok(args)
}

fn require(flag: &str, value: Option<String>) -> Result<String, String> {
    value.ok_or_else(|| format!("missing required flag: {flag}"))
}

fn parse_endpoint(s: &str) -> Result<SocketAddr, String> {
    s.parse().map_err(|e| format!("invalid endpoint {s}: {e}"))
}

fn build_config(args: Args) -> Result<(ProofConfig, Option<String>), String> {
    let host_endpoint = parse_endpoint("127.0.0.1:41000")
        .map_err(|e| format!("default host endpoint invalid: {e}"))?;
    let guest_endpoint = parse_endpoint("127.0.0.1:41001")
        .map_err(|e| format!("default guest endpoint invalid: {e}"))?;
    let config = ProofConfig {
        room_id: require("--room", args.room)?,
        game_id: require("--game", args.game)?,
        host_user_id: require("--host-user", args.host_user)?,
        guest_user_id: require("--guest-user", args.guest_user)?,
        host_endpoint,
        guest_endpoint,
        input_delay_frames: 0,
        platform: require("--platform", args.platform)?,
    };
    Ok((config, args.out))
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("error: {err}");
            eprintln!();
            eprintln!("{USAGE}");
            return ExitCode::from(1);
        }
    };
    if args.help {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    let (config, out) = match build_config(args) {
        Ok(tuple) => tuple,
        Err(err) => {
            eprintln!("error: {err}");
            eprintln!();
            eprintln!("{USAGE}");
            return ExitCode::from(1);
        }
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("error: failed to start runtime: {err}");
            return ExitCode::from(1);
        }
    };

    match runtime.block_on(run_proof(&config)) {
        Ok(report) => {
            let payload = serde_json::json!({
                "host": report.host,
                "guest": report.guest,
                "verification": report.verification,
            });
            let pretty = match serde_json::to_string_pretty(&payload) {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("error: failed to serialize evidence: {err}");
                    return ExitCode::from(1);
                }
            };
            if let Some(path) = out.as_deref() {
                if let Err(err) = std::fs::write(path, &pretty) {
                    eprintln!("error: failed to write {path}: {err}");
                    return ExitCode::from(1);
                }
            } else {
                println!("{pretty}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {}: {}", error.code(), error);
            ExitCode::from(1)
        }
    }
}
