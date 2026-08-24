use opencade_protocol::{
    MatchReport, MatchReportTransport, RoomState, MATCH_REPORT_SCHEMA_VERSION,
};
use serde::Serialize;

pub const ALPHA_MATCH_FRAMES: u32 = 60;

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

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReportVerificationError {
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
}
