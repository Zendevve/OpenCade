//! Integration test for the `opencade-proof-of-match` crate.
//!
//! These tests are intentionally black-box: only the public API of the new crate and the
//! existing public re-exports of `opencade_networking` and `opencade_protocol` are used.

use opencade_networking::verify_match_reports;
use opencade_proof_of_match::{ProofConfig, run_proof};
use opencade_protocol::RoomState;

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

#[tokio::test(flavor = "current_thread")]
async fn two_mock_peers_produce_a_verified_60_frame_transcript() {
    let report = run_proof(&canonical_config())
        .await
        .expect("proof succeeds");
    assert!(
        report.verification.verified,
        "verification should pass for the canonical scenario"
    );
    assert_eq!(report.host.probe.frames_received, 60);
    assert_eq!(report.guest.probe.frames_received, 60);
    assert_eq!(
        report.host.probe.transcript_checksum, report.guest.probe.transcript_checksum,
        "transcript checksums must be byte-identical"
    );
    assert_ne!(report.host.probe.role, report.guest.probe.role);
    assert_eq!(report.host.room.state, RoomState::Finished);
    assert_eq!(report.guest.room.state, RoomState::Finished);
    assert_eq!(report.host.schema_version, 1);
    assert_eq!(report.guest.schema_version, 1);
    assert_eq!(report.host.room.id, "demo");
    assert_eq!(report.guest.room.id, "demo");
    assert_eq!(report.host.room.game_id, "sfiii3");
    assert_eq!(report.guest.room.game_id, "sfiii3");
}

#[tokio::test(flavor = "current_thread")]
async fn rejects_mismatched_transcripts() {
    let report = run_proof(&canonical_config())
        .await
        .expect("proof succeeds");
    let mut tampered = report.host.clone();
    let original = tampered.probe.transcript_checksum.clone();
    assert_eq!(original.len(), 16, "checksum must be 16 hex chars");

    // Replace the last character with a different valid lowercase hex digit. The test does
    // not care which one — only that the resulting string differs and is still a 16-hex string
    // (so the rejection comes from `ChecksumMismatch`, not `InvalidChecksum`).
    let last = original.chars().last().expect("non-empty");
    let replacement = match last {
        '0'..='e' => ((last as u8) + 1) as char,
        'f' => '0',
        other => panic!("unexpected non-hex char: {other}"),
    };
    let mut chars: Vec<char> = original.chars().collect();
    let last_idx = chars.len() - 1;
    chars[last_idx] = replacement;
    let flipped_last: String = chars.into_iter().collect();
    assert_ne!(flipped_last, original);
    tampered.probe.transcript_checksum = flipped_last;

    let result = verify_match_reports(&tampered, &report.guest);
    let err = result.expect_err("mismatched checksum must be rejected");
    assert_eq!(err.code(), "checksum_mismatch");
}
