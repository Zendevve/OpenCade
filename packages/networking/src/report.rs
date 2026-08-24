use opencade_protocol::{
    ALPHA_FAILURE_REPORT_SCHEMA_VERSION, AlphaFailureReport, MATCH_REPORT_SCHEMA_VERSION,
    MatchCandidateKind, MatchReport, MatchReportTransport, NatMappingState, RoomState,
};
use serde::{Serialize, de::DeserializeOwned};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const ALPHA_MATCH_FRAMES: u32 = 60;
pub const MAX_REPORT_BYTES: u64 = 64 * 1024;

type CompatibilityKey = (String, String, &'static str, &'static str, &'static str);
type CompatibilityCounts = (usize, usize);

#[derive(Debug, thiserror::Error)]
pub enum ReportReadError {
    #[error("report could not be read")]
    Unreadable,
    #[error("report exceeds 64 KiB")]
    TooLarge,
    #[error("report is not canonical OpenCade alpha evidence")]
    Invalid,
}

impl ReportReadError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unreadable => "report_unreadable",
            Self::TooLarge => "report_too_large",
            Self::Invalid => "report_invalid",
        }
    }
}

pub fn read_match_report(path: &Path) -> Result<MatchReport, ReportReadError> {
    read_bounded_json(path)
}

#[derive(Debug)]
pub enum AlphaCampaignEvidence {
    Match(MatchReport),
    Failure(AlphaFailureReport),
}

pub fn read_campaign_evidence(path: &Path) -> Result<AlphaCampaignEvidence, ReportReadError> {
    let bytes = read_bounded_bytes(path)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| ReportReadError::Invalid)?;
    match value.get("kind") {
        None => serde_json::from_value(value)
            .map(AlphaCampaignEvidence::Match)
            .map_err(|_| ReportReadError::Invalid),
        Some(serde_json::Value::String(kind)) if kind == "attempt_failure" => {
            serde_json::from_value(value)
                .map(AlphaCampaignEvidence::Failure)
                .map_err(|_| ReportReadError::Invalid)
        }
        Some(_) => Err(ReportReadError::Invalid),
    }
}

fn read_bounded_json<T: DeserializeOwned>(path: &Path) -> Result<T, ReportReadError> {
    let bytes = read_bounded_bytes(path)?;
    serde_json::from_slice(&bytes).map_err(|_| ReportReadError::Invalid)
}

fn read_bounded_bytes(path: &Path) -> Result<Vec<u8>, ReportReadError> {
    let file = File::open(path).map_err(|_| ReportReadError::Unreadable)?;
    let mut bytes = Vec::new();
    file.take(MAX_REPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReportReadError::Unreadable)?;
    if bytes.len() as u64 > MAX_REPORT_BYTES {
        return Err(ReportReadError::TooLarge);
    }
    Ok(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchVerification {
    pub schema_version: u8,
    pub verified: bool,
    pub room_id: String,
    pub game_id: String,
    pub transport: MatchReportTransport,
    pub frames_received: u32,
    pub transcript_checksum: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlphaCampaignFailure {
    pub room_id: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompatibilityResult {
    pub game_id: String,
    pub platform: String,
    pub transport: &'static str,
    pub nat: &'static str,
    pub candidate: &'static str,
    pub attempts: usize,
    pub verified: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlphaCampaignSummary {
    pub schema_version: u8,
    pub reports: usize,
    pub attempts: usize,
    pub verified: usize,
    pub failed: usize,
    pub success_rate: f64,
    pub gate_passed: bool,
    pub failures: Vec<AlphaCampaignFailure>,
    pub compatibility: Vec<CompatibilityResult>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReportVerificationError {
    #[error("room has {reports} reports; exactly two are required")]
    PairSize { reports: usize },
    #[error("{report} report uses unsupported schema version {version}")]
    UnsupportedSchema { report: &'static str, version: u8 },
    #[error("{report} report does not describe a finished room")]
    RoomNotFinished { report: &'static str },
    #[error("{report} report has an empty room or game identifier")]
    EmptyCorrelation { report: &'static str },
    #[error("{report} report must contain exactly {ALPHA_MATCH_FRAMES} received frames")]
    IncompleteTranscript { report: &'static str },
    #[error("{report} report sent fewer frames than it received")]
    InvalidFrameCounts { report: &'static str },
    #[error("{report} report checksum must be 16 lowercase hexadecimal characters")]
    InvalidChecksum { report: &'static str },
    #[error("reports describe different rooms")]
    RoomMismatch,
    #[error("reports describe different games")]
    GameMismatch,
    #[error("reports must come from opposite host and guest roles")]
    RoleMismatch,
    #[error("reports contain different transcript checksums")]
    ChecksumMismatch,
    #[error("reports describe different transports")]
    TransportMismatch,
    #[error("reports contain different native emulator compatibility fingerprints")]
    CompatibilityMismatch,
    #[error("{report} report is missing native emulator compatibility evidence")]
    CompatibilityMissing { report: &'static str },
    #[error("{report} report contains invalid native emulator compatibility evidence")]
    InvalidCompatibility { report: &'static str },
}

impl ReportVerificationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::PairSize { .. } => "report_pair_invalid",
            Self::UnsupportedSchema { .. } => "schema_unsupported",
            Self::RoomNotFinished { .. } => "room_not_finished",
            Self::EmptyCorrelation { .. } => "correlation_missing",
            Self::IncompleteTranscript { .. } => "transcript_incomplete",
            Self::InvalidFrameCounts { .. } => "frame_counts_invalid",
            Self::InvalidChecksum { .. } => "checksum_invalid",
            Self::RoomMismatch => "room_mismatch",
            Self::GameMismatch => "game_mismatch",
            Self::RoleMismatch => "role_mismatch",
            Self::ChecksumMismatch => "checksum_mismatch",
            Self::TransportMismatch => "transport_mismatch",
            Self::CompatibilityMismatch => "compatibility_mismatch",
            Self::CompatibilityMissing { .. } => "compatibility_missing",
            Self::InvalidCompatibility { .. } => "compatibility_invalid",
        }
    }
}

pub fn summarize_match_reports(reports: &[MatchReport]) -> AlphaCampaignSummary {
    summarize_campaign_evidence(reports, &[])
}

pub fn summarize_campaign_evidence(
    reports: &[MatchReport],
    failure_reports: &[AlphaFailureReport],
) -> AlphaCampaignSummary {
    let mut rooms: BTreeMap<String, Vec<&MatchReport>> = BTreeMap::new();
    for report in reports {
        rooms
            .entry(report.room.id.clone())
            .or_default()
            .push(report);
    }
    let mut failure_rooms: BTreeMap<String, Vec<&AlphaFailureReport>> = BTreeMap::new();
    for report in failure_reports {
        failure_rooms
            .entry(report.room.id.clone())
            .or_default()
            .push(report);
    }
    let room_ids: BTreeSet<String> = rooms.keys().chain(failure_rooms.keys()).cloned().collect();

    let mut verified = 0;
    let mut failures = Vec::new();
    let mut compatibility: BTreeMap<CompatibilityKey, CompatibilityCounts> = BTreeMap::new();

    for room_id in &room_ids {
        let pair = rooms.get(room_id).map(Vec::as_slice).unwrap_or_default();
        let abandoned = failure_rooms
            .get(room_id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let result = if !abandoned.is_empty() {
            failure_result(pair, abandoned)
        } else if pair.len() == 2 {
            verify_match_reports(pair[0], pair[1])
                .map(|_| ())
                .map_err(|error| error.code().to_string())
        } else {
            Err("report_pair_invalid".to_string())
        };
        let is_verified = result.is_ok();
        if is_verified {
            verified += 1;
        } else if let Err(code) = result {
            failures.push(AlphaCampaignFailure {
                room_id: room_id.clone(),
                code,
            });
        }

        let mut seen = BTreeSet::new();
        for report in pair {
            let key = (
                report.room.game_id.clone(),
                report.client.platform.clone(),
                transport_label(report.probe.transport),
                nat_label(report.probe.nat),
                candidate_label(report.probe.candidate),
            );
            if seen.insert(key.clone()) {
                let counts = compatibility.entry(key).or_default();
                counts.0 += 1;
                if is_verified {
                    counts.1 += 1;
                }
            }
        }
        for report in abandoned {
            let key = (
                report.room.game_id.clone(),
                report.client.platform.clone(),
                optional_transport_label(report.transport),
                "unknown",
                "unknown",
            );
            if seen.insert(key.clone()) {
                compatibility.entry(key).or_default().0 += 1;
            }
        }
    }

    let attempts = room_ids.len();
    let success_rate = if attempts == 0 {
        0.0
    } else {
        verified as f64 / attempts as f64
    };
    AlphaCampaignSummary {
        schema_version: 1,
        reports: reports.len() + failure_reports.len(),
        attempts,
        verified,
        failed: attempts - verified,
        success_rate,
        gate_passed: attempts >= 10 && success_rate >= 0.8,
        failures,
        compatibility: compatibility
            .into_iter()
            .map(
                |((game_id, platform, transport, nat, candidate), (attempts, verified))| {
                    CompatibilityResult {
                        game_id,
                        platform,
                        transport,
                        nat,
                        candidate,
                        attempts,
                        verified,
                    }
                },
            )
            .collect(),
    }
}

fn failure_result(
    matches: &[&MatchReport],
    failures: &[&AlphaFailureReport],
) -> Result<(), String> {
    if !matches.is_empty() {
        return Err("evidence_conflict".into());
    }
    for report in failures {
        validate_failure_report(report).map_err(|error| error.code().to_string())?;
    }
    if failures
        .iter()
        .map(|report| report.room.game_id.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != 1
    {
        return Err("evidence_conflict".into());
    }
    Err(failures
        .iter()
        .map(|report| report.error_code.as_str())
        .min()
        .unwrap_or("attempt_failed")
        .to_string())
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
enum FailureEvidenceError {
    #[error("failure report uses an unsupported schema version")]
    UnsupportedSchema,
    #[error("failure report has an empty room or game identifier")]
    EmptyCorrelation,
    #[error("failure report has an invalid error code")]
    InvalidCode,
}

impl FailureEvidenceError {
    fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedSchema => "schema_unsupported",
            Self::EmptyCorrelation => "correlation_missing",
            Self::InvalidCode => "failure_code_invalid",
        }
    }
}

fn validate_failure_report(report: &AlphaFailureReport) -> Result<(), FailureEvidenceError> {
    if report.schema_version != ALPHA_FAILURE_REPORT_SCHEMA_VERSION {
        return Err(FailureEvidenceError::UnsupportedSchema);
    }
    if report.room.id.trim().is_empty() || report.room.game_id.trim().is_empty() {
        return Err(FailureEvidenceError::EmptyCorrelation);
    }
    let valid_code = (3..=64).contains(&report.error_code.len())
        && report
            .error_code
            .bytes()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == b'_');
    if !valid_code {
        return Err(FailureEvidenceError::InvalidCode);
    }
    Ok(())
}

fn transport_label(value: MatchReportTransport) -> &'static str {
    match value {
        MatchReportTransport::DirectUdp => "direct_udp",
        MatchReportTransport::Relay => "relay",
    }
}

fn optional_transport_label(value: Option<MatchReportTransport>) -> &'static str {
    value.map(transport_label).unwrap_or("not_selected")
}

fn nat_label(value: Option<NatMappingState>) -> &'static str {
    match value {
        Some(NatMappingState::Open) => "open",
        Some(NatMappingState::Mapped) => "mapped",
        Some(NatMappingState::Unknown) | None => "unknown",
    }
}

fn candidate_label(value: Option<MatchCandidateKind>) -> &'static str {
    match value {
        Some(MatchCandidateKind::Host) => "host",
        Some(MatchCandidateKind::Reflexive) => "reflexive",
        None => "unknown",
    }
}

pub fn verify_match_reports(
    first: &MatchReport,
    second: &MatchReport,
) -> Result<MatchVerification, ReportVerificationError> {
    validate_report("first", first)?;
    validate_report("second", second)?;

    if first.room.id != second.room.id {
        return Err(ReportVerificationError::RoomMismatch);
    }
    if first.room.game_id != second.room.game_id {
        return Err(ReportVerificationError::GameMismatch);
    }
    if first.probe.role == second.probe.role {
        return Err(ReportVerificationError::RoleMismatch);
    }
    if first.probe.transcript_checksum != second.probe.transcript_checksum {
        return Err(ReportVerificationError::ChecksumMismatch);
    }
    if first.probe.transport != second.probe.transport {
        return Err(ReportVerificationError::TransportMismatch);
    }
    if first.compatibility != second.compatibility {
        return Err(ReportVerificationError::CompatibilityMismatch);
    }

    Ok(MatchVerification {
        schema_version: MATCH_REPORT_SCHEMA_VERSION,
        verified: true,
        room_id: first.room.id.clone(),
        game_id: first.room.game_id.clone(),
        transport: first.probe.transport,
        frames_received: ALPHA_MATCH_FRAMES,
        transcript_checksum: first.probe.transcript_checksum.clone(),
    })
}

pub fn verify_playable_match_reports(
    first: &MatchReport,
    second: &MatchReport,
) -> Result<MatchVerification, ReportVerificationError> {
    if first.compatibility.is_none() {
        return Err(ReportVerificationError::CompatibilityMissing { report: "first" });
    }
    if second.compatibility.is_none() {
        return Err(ReportVerificationError::CompatibilityMissing { report: "second" });
    }
    verify_match_reports(first, second)
}

fn validate_report(
    label: &'static str,
    report: &MatchReport,
) -> Result<(), ReportVerificationError> {
    if report.schema_version != MATCH_REPORT_SCHEMA_VERSION {
        return Err(ReportVerificationError::UnsupportedSchema {
            report: label,
            version: report.schema_version,
        });
    }
    if report.room.state != RoomState::Finished {
        return Err(ReportVerificationError::RoomNotFinished { report: label });
    }
    if report.room.id.trim().is_empty() || report.room.game_id.trim().is_empty() {
        return Err(ReportVerificationError::EmptyCorrelation { report: label });
    }
    if report.probe.frames_received != ALPHA_MATCH_FRAMES {
        return Err(ReportVerificationError::IncompleteTranscript { report: label });
    }
    if report.probe.frames_sent < report.probe.frames_received {
        return Err(ReportVerificationError::InvalidFrameCounts { report: label });
    }
    let checksum = report.probe.transcript_checksum.as_bytes();
    if checksum.len() != 16
        || !checksum
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(ReportVerificationError::InvalidChecksum { report: label });
    }
    if let Some(compatibility) = &report.compatibility {
        let hashes = [
            &compatibility.executable_sha256,
            &compatibility.core_sha256,
            &compatibility.content_sha256,
        ];
        let valid_hashes = hashes.iter().all(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        });
        if compatibility.adapter.trim().is_empty()
            || compatibility
                .emulator_version
                .as_ref()
                .is_some_and(|version| version.trim().is_empty())
            || !valid_hashes
        {
            return Err(ReportVerificationError::InvalidCompatibility { report: label });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use opencade_protocol::{
        AlphaEvidenceKind, AlphaFailureStage, MatchReportClient, MatchReportCompatibility,
        MatchReportProbe, MatchReportRole, MatchReportRoom,
    };

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
                frames_received: ALPHA_MATCH_FRAMES,
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

    fn failure_report() -> AlphaFailureReport {
        AlphaFailureReport {
            schema_version: ALPHA_FAILURE_REPORT_SCHEMA_VERSION,
            kind: AlphaEvidenceKind::AttemptFailure,
            exported_at: Utc
                .with_ymd_and_hms(2026, 8, 24, 12, 0, 0)
                .single()
                .expect("timestamp"),
            room: MatchReportRoom {
                id: "failed-room".into(),
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

    #[test]
    fn verifies_complementary_complete_reports() {
        let verified = verify_match_reports(
            &report(MatchReportRole::Host),
            &report(MatchReportRole::Guest),
        )
        .expect("paired reports");

        assert!(verified.verified);
        assert_eq!(verified.room_id, "room-1");
        assert_eq!(verified.frames_received, ALPHA_MATCH_FRAMES);
    }

    #[test]
    fn rejects_same_role_or_mismatched_checksum() {
        let host = report(MatchReportRole::Host);
        assert_eq!(
            verify_match_reports(&host, &host),
            Err(ReportVerificationError::RoleMismatch)
        );

        let mut guest = report(MatchReportRole::Guest);
        guest.probe.transcript_checksum = "aaaaaaaaaaaaaaaa".into();
        assert_eq!(
            verify_match_reports(&host, &guest),
            Err(ReportVerificationError::ChecksumMismatch)
        );
    }

    #[test]
    fn rejects_incomplete_or_noncanonical_reports() {
        let host = report(MatchReportRole::Host);
        let mut guest = report(MatchReportRole::Guest);
        guest.probe.frames_received = 59;
        assert_eq!(
            verify_match_reports(&host, &guest),
            Err(ReportVerificationError::IncompleteTranscript { report: "second" })
        );

        guest = report(MatchReportRole::Guest);
        guest.schema_version = 2;
        assert_eq!(
            verify_match_reports(&host, &guest),
            Err(ReportVerificationError::UnsupportedSchema {
                report: "second",
                version: 2,
            })
        );
    }

    #[test]
    fn rejects_transport_or_native_fingerprint_mismatches() {
        let host = report(MatchReportRole::Host);
        let mut guest = report(MatchReportRole::Guest);
        guest.probe.transport = MatchReportTransport::Relay;
        assert_eq!(
            verify_match_reports(&host, &guest),
            Err(ReportVerificationError::TransportMismatch)
        );

        guest = report(MatchReportRole::Guest);
        guest.compatibility = Some(MatchReportCompatibility {
            adapter: "retroarch_fbneo".into(),
            emulator_version: Some("1.22.0".into()),
            executable_sha256: "a".repeat(64),
            core_sha256: "b".repeat(64),
            content_sha256: "c".repeat(64),
        });
        assert_eq!(
            verify_match_reports(&host, &guest),
            Err(ReportVerificationError::CompatibilityMismatch)
        );
    }

    #[test]
    fn strict_playable_verification_requires_valid_native_evidence() {
        let host = report(MatchReportRole::Host);
        let guest = report(MatchReportRole::Guest);
        assert_eq!(
            verify_playable_match_reports(&host, &guest),
            Err(ReportVerificationError::CompatibilityMissing { report: "first" })
        );

        let compatibility = MatchReportCompatibility {
            adapter: "retroarch_fbneo".into(),
            emulator_version: Some("1.22.0".into()),
            executable_sha256: "a".repeat(64),
            core_sha256: "b".repeat(64),
            content_sha256: "c".repeat(64),
        };
        let mut host = host;
        let mut guest = guest;
        host.compatibility = Some(compatibility.clone());
        guest.compatibility = Some(compatibility);
        assert!(verify_playable_match_reports(&host, &guest).is_ok());

        guest
            .compatibility
            .as_mut()
            .expect("compatibility")
            .core_sha256 = "NOT-A-HASH".into();
        assert_eq!(
            verify_playable_match_reports(&host, &guest),
            Err(ReportVerificationError::InvalidCompatibility { report: "second" })
        );
    }

    #[test]
    fn campaign_gate_accepts_eight_of_ten_verified_attempts() {
        let mut reports = Vec::new();
        for attempt in 0..10 {
            let room_id = format!("room-{attempt}");
            let mut host = report(MatchReportRole::Host);
            host.room.id.clone_from(&room_id);
            let mut guest = report(MatchReportRole::Guest);
            guest.room.id = room_id;
            if attempt >= 8 {
                guest.probe.transcript_checksum = "aaaaaaaaaaaaaaaa".into();
            }
            reports.extend([host, guest]);
        }

        let summary = summarize_match_reports(&reports);
        assert_eq!(summary.attempts, 10);
        assert_eq!(summary.verified, 8);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.success_rate, 0.8);
        assert!(summary.gate_passed);
        assert_eq!(summary.compatibility.len(), 1);
        assert_eq!(summary.compatibility[0].transport, "direct_udp");
        assert_eq!(summary.compatibility[0].verified, 8);
    }

    #[test]
    fn campaign_matrix_separates_direct_and_relay_attempts() {
        let mut reports = Vec::new();
        for (room_id, transport) in [
            ("direct-room", MatchReportTransport::DirectUdp),
            ("relay-room", MatchReportTransport::Relay),
        ] {
            for role in [MatchReportRole::Host, MatchReportRole::Guest] {
                let mut value = report(role);
                value.room.id = room_id.into();
                value.probe.transport = transport;
                if transport == MatchReportTransport::Relay {
                    value.probe.candidate = None;
                    value.probe.punch_attempts = None;
                }
                reports.push(value);
            }
        }

        let summary = summarize_match_reports(&reports);
        assert_eq!(summary.verified, 2);
        assert_eq!(summary.compatibility.len(), 2);
        assert_eq!(summary.compatibility[0].transport, "direct_udp");
        assert_eq!(summary.compatibility[1].transport, "relay");
    }

    #[test]
    fn campaign_counts_an_abandoned_room_without_success_reports() {
        let summary = summarize_campaign_evidence(&[], &[failure_report()]);

        assert_eq!(summary.reports, 1);
        assert_eq!(summary.attempts, 1);
        assert_eq!(summary.verified, 0);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.failures[0].code, "relay_timeout");
        assert_eq!(summary.compatibility[0].transport, "relay");
        assert_eq!(summary.compatibility[0].verified, 0);
    }

    #[test]
    fn campaign_rejects_success_and_failure_evidence_for_one_room() {
        let host = report(MatchReportRole::Host);
        let guest = report(MatchReportRole::Guest);
        let mut failure = failure_report();
        failure.room.id = host.room.id.clone();

        let summary = summarize_campaign_evidence(&[host, guest], &[failure]);

        assert_eq!(summary.attempts, 1);
        assert_eq!(summary.verified, 0);
        assert_eq!(summary.failures[0].code, "evidence_conflict");
    }

    #[test]
    fn campaign_rejects_unstable_failure_codes() {
        let mut failure = failure_report();
        failure.error_code = "Relay timed out at C:\\Users\\tester".into();

        let summary = summarize_campaign_evidence(&[], &[failure]);

        assert_eq!(summary.failed, 1);
        assert_eq!(summary.failures[0].code, "failure_code_invalid");
    }
}
