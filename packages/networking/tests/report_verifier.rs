use chrono::{TimeZone, Utc};
use opencade_protocol::{
    MatchReport, MatchReportClient, MatchReportProbe, MatchReportRole, MatchReportRoom,
    MatchReportTransport, RoomState, MATCH_REPORT_SCHEMA_VERSION,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn report(role: MatchReportRole) -> MatchReport {
    MatchReport {
        schema_version: MATCH_REPORT_SCHEMA_VERSION,
        exported_at: Utc
            .with_ymd_and_hms(2026, 8, 24, 12, 0, 0)
            .single()
            .expect("timestamp"),
        room: MatchReportRoom {
            id: "room-1".into(),
            game_id: "sfiii3".into(),
            state: RoomState::Finished,
        },
        probe: MatchReportProbe {
            role,
            transport: MatchReportTransport::DirectUdp,
            frames_sent: 64,
            frames_received: 60,
            transcript_checksum: "0376c2e852f4fd25".into(),
            elapsed_ms: 240,
        },
        client: MatchReportClient {
            platform: "windows".into(),
            user_agent: "opencade-test".into(),
        },
    }
}

fn fixture_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "opencade-report-test-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn verifier_cli_returns_machine_readable_success_and_failure() {
    let directory = fixture_dir();
    fs::create_dir(&directory).expect("fixture directory");
    let host_path = directory.join("host.json");
    let guest_path = directory.join("guest.json");
    fs::write(
        &host_path,
        serde_json::to_vec(&report(MatchReportRole::Host)).expect("host JSON"),
    )
    .expect("host report");
    fs::write(
        &guest_path,
        serde_json::to_vec(&report(MatchReportRole::Guest)).expect("guest JSON"),
    )
    .expect("guest report");

    let success = Command::new(env!("CARGO_BIN_EXE_opencade-match-verify"))
        .arg(&host_path)
        .arg(&guest_path)
        .output()
        .expect("run verifier");
    assert!(success.status.success());
    let output: serde_json::Value =
        serde_json::from_slice(&success.stdout).expect("success output JSON");
    assert_eq!(output["verified"], true);
    assert_eq!(output["room_id"], "room-1");

    let mut mismatched = report(MatchReportRole::Guest);
    mismatched.probe.transcript_checksum = "aaaaaaaaaaaaaaaa".into();
    fs::write(
        &guest_path,
        serde_json::to_vec(&mismatched).expect("mismatch JSON"),
    )
    .expect("mismatched report");
    let failure = Command::new(env!("CARGO_BIN_EXE_opencade-match-verify"))
        .arg(&host_path)
        .arg(&guest_path)
        .output()
        .expect("run verifier mismatch");
    assert_eq!(failure.status.code(), Some(1));
    let output: serde_json::Value =
        serde_json::from_slice(&failure.stderr).expect("failure output JSON");
    assert_eq!(output["verified"], false);
    assert_eq!(output["code"], "checksum_mismatch");

    fs::write(&guest_path, vec![b' '; 64 * 1024 + 1]).expect("oversized report");
    let oversized = Command::new(env!("CARGO_BIN_EXE_opencade-match-verify"))
        .arg(&host_path)
        .arg(&guest_path)
        .output()
        .expect("run verifier with oversized report");
    assert_eq!(oversized.status.code(), Some(2));
    let output: serde_json::Value =
        serde_json::from_slice(&oversized.stderr).expect("oversized output JSON");
    assert_eq!(output["code"], "report_too_large");

    fs::remove_dir_all(directory).expect("remove fixture directory");
}
