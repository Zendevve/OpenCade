use chrono::{TimeZone, Utc};
use opencade_protocol::{
    ALPHA_FAILURE_REPORT_SCHEMA_VERSION, AlphaEvidenceKind, AlphaFailureReport, AlphaFailureStage,
    MATCH_REPORT_SCHEMA_VERSION, MatchReport, MatchReportClient, MatchReportProbe, MatchReportRole,
    MatchReportRoom, MatchReportTransport, RoomState,
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
            nat: Some(opencade_protocol::NatMappingState::Mapped),
            candidate: Some(opencade_protocol::MatchCandidateKind::Reflexive),
            punch_attempts: Some(2),
        },
        client: MatchReportClient {
            platform: "windows".into(),
            user_agent: "opencade-test".into(),
        },
        compatibility: None,
    }
}

fn failure_report(room_id: String) -> AlphaFailureReport {
    AlphaFailureReport {
        schema_version: ALPHA_FAILURE_REPORT_SCHEMA_VERSION,
        kind: AlphaEvidenceKind::AttemptFailure,
        exported_at: Utc
            .with_ymd_and_hms(2026, 8, 24, 12, 0, 0)
            .single()
            .expect("timestamp"),
        room: MatchReportRoom {
            id: room_id,
            game_id: "sfiii3".into(),
            state: RoomState::Connecting,
        },
        role: MatchReportRole::Host,
        stage: AlphaFailureStage::Relay,
        error_code: "relay_timeout".into(),
        transport: Some(MatchReportTransport::Relay),
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

    let strict = Command::new(env!("CARGO_BIN_EXE_opencade-match-verify"))
        .arg("--require-compatibility")
        .arg(&host_path)
        .arg(&guest_path)
        .output()
        .expect("run strict verifier");
    assert_eq!(strict.status.code(), Some(1));
    let output: serde_json::Value =
        serde_json::from_slice(&strict.stderr).expect("strict failure output JSON");
    assert_eq!(output["code"], "compatibility_missing");

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

#[test]
fn summary_cli_derives_eight_of_ten_campaign_gate() {
    let directory = fixture_dir();
    fs::create_dir(&directory).expect("fixture directory");

    for attempt in 0..8 {
        let room_id = format!("room-{attempt}");
        let mut host = report(MatchReportRole::Host);
        host.room.id.clone_from(&room_id);
        let mut guest = report(MatchReportRole::Guest);
        guest.room.id = room_id;
        for (role, report) in [("host", host), ("guest", guest)] {
            let path = directory.join(format!("attempt-{attempt:02}-{role}.json"));
            fs::write(path, serde_json::to_vec(&report).expect("report JSON"))
                .expect("campaign report");
        }
    }
    for attempt in 8..10 {
        let path = directory.join(format!("attempt-{attempt:02}-failure.json"));
        let failure = failure_report(format!("room-{attempt}"));
        fs::write(path, serde_json::to_vec(&failure).expect("failure JSON"))
            .expect("campaign failure report");
    }

    let result = Command::new(env!("CARGO_BIN_EXE_opencade-alpha-summary"))
        .arg(&directory)
        .output()
        .expect("run campaign summary");
    assert!(result.status.success());
    let output: serde_json::Value =
        serde_json::from_slice(&result.stdout).expect("summary output JSON");
    assert_eq!(output["attempts"], 10);
    assert_eq!(output["reports"], 18);
    assert_eq!(output["verified"], 8);
    assert_eq!(output["success_rate"], 0.8);
    assert_eq!(output["gate_passed"], true);
    assert_eq!(output["compatibility"][0]["attempts"], 8);
    assert_eq!(output["compatibility"][0]["verified"], 8);
    assert_eq!(output["compatibility"][0]["transport"], "direct_udp");
    assert_eq!(output["compatibility"][1]["attempts"], 2);
    assert_eq!(output["compatibility"][1]["verified"], 0);
    assert_eq!(output["compatibility"][1]["transport"], "relay");

    fs::remove_dir_all(directory).expect("remove fixture directory");
}

#[test]
fn summary_cli_rejects_unknown_evidence_kinds() {
    let directory = fixture_dir();
    fs::create_dir(&directory).expect("fixture directory");
    fs::write(
        directory.join("unknown.json"),
        br#"{"schema_version":1,"kind":"future_evidence"}"#,
    )
    .expect("unknown evidence");

    let result = Command::new(env!("CARGO_BIN_EXE_opencade-alpha-summary"))
        .arg(&directory)
        .output()
        .expect("run campaign summary");

    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("report_invalid"));
    fs::remove_dir_all(directory).expect("remove fixture directory");
}
