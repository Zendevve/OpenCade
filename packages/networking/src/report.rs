use opencade_protocol::{
    MatchCandidateKind, MatchReport, MatchReportTransport, NatMappingState, RoomState,
    MATCH_REPORT_SCHEMA_VERSION,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const ALPHA_MATCH_FRAMES: u32 = 60;
pub const MAX_REPORT_BYTES: u64 = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ReportReadError {
    #[error("report could not be read")]
    Unreadable,
    #[error("report exceeds 64 KiB")]
    TooLarge,
    #[error("report is not a canonical OpenCade match report")]
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
    let file = File::open(path).map_err(|_| ReportReadError::Unreadable)?;
    let mut bytes = Vec::new();
    file.take(MAX_REPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReportReadError::Unreadable)?;
    if bytes.len() as u64 > MAX_REPORT_BYTES {
        return Err(ReportReadError::TooLarge);
    }
    serde_json::from_slice(&bytes).map_err(|_| ReportReadError::Invalid)
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
    pub code: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompatibilityResult {
    pub game_id: String,
    pub platform: String,
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
        }
    }
}

pub fn summarize_match_reports(reports: &[MatchReport]) -> AlphaCampaignSummary {
    let mut rooms: BTreeMap<&str, Vec<&MatchReport>> = BTreeMap::new();
    for report in reports {
        rooms.entry(&report.room.id).or_default().push(report);
    }

    let mut verified = 0;
    let mut failures = Vec::new();
    let mut compatibility: BTreeMap<(String, String, &'static str, &'static str), (usize, usize)> =
        BTreeMap::new();

    for (room_id, pair) in &rooms {
        let result = if pair.len() == 2 {
            verify_match_reports(pair[0], pair[1])
        } else {
            Err(ReportVerificationError::PairSize {
                reports: pair.len(),
            })
        };
        let is_verified = result.is_ok();
        if is_verified {
            verified += 1;
        } else if let Err(error) = result {
            failures.push(AlphaCampaignFailure {
                room_id: (*room_id).to_string(),
                code: error.code(),
            });
        }

        let mut seen = BTreeSet::new();
        for report in pair {
            let key = (
                report.room.game_id.clone(),
                report.client.platform.clone(),
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
    }

    let attempts = rooms.len();
    let success_rate = if attempts == 0 {
        0.0
    } else {
        verified as f64 / attempts as f64
    };
    AlphaCampaignSummary {
        schema_version: 1,
        reports: reports.len(),
        attempts,
        verified,
        failed: attempts - verified,
        success_rate,
        gate_passed: attempts >= 10 && success_rate >= 0.8,
        failures,
        compatibility: compatibility
            .into_iter()
            .map(
                |((game_id, platform, nat, candidate), (attempts, verified))| CompatibilityResult {
                    game_id,
                    platform,
                    nat,
                    candidate,
                    attempts,
                    verified,
                },
            )
            .collect(),
    }
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

    Ok(MatchVerification {
        schema_version: MATCH_REPORT_SCHEMA_VERSION,
        verified: true,
        room_id: first.room.id.clone(),
        game_id: first.room.game_id.clone(),
        transport: MatchReportTransport::DirectUdp,
        frames_received: ALPHA_MATCH_FRAMES,
        transcript_checksum: first.probe.transcript_checksum.clone(),
    })
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use opencade_protocol::{
        MatchReportClient, MatchReportProbe, MatchReportRole, MatchReportRoom,
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
    fn campaign_gate_accepts_eight_of_ten_verified_attempts() {
        let mut reports = Vec::new();
        for attempt in 0..10 {
            let room_id = format!("room-{attempt}");
            let mut host = report(MatchReportRole::Host);
            host.room.id = room_id.clone();
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
        assert_eq!(summary.compatibility[0].verified, 8);
    }
}
