use opencade_protocol::{NativeRouteCapability, NativeRoutePolicyPayload};
use sqlx::{PgPool, Row};

use crate::error::AppError;

const REQUIRED_SUCCESS_PERCENT: u32 = 80;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RouteEvidence {
    pub attempts: u32,
    pub verified: u32,
}

#[must_use]
pub fn decide_native_route(
    tunnel_enabled: bool,
    minimum_attempts: u32,
    tunnel: RouteEvidence,
) -> NativeRoutePolicyPayload {
    let enough_samples = tunnel.attempts >= minimum_attempts;
    let meets_success_gate = tunnel.attempts > 0
        && tunnel.verified.saturating_mul(100)
            >= tunnel.attempts.saturating_mul(REQUIRED_SUCCESS_PERCENT);
    if tunnel_enabled && enough_samples && meets_success_gate {
        return NativeRoutePolicyPayload {
            route: NativeRouteCapability::TcpTunnel,
            reason: "physical_evidence_gate_passed".into(),
            evidence_attempts: tunnel.attempts,
            evidence_verified: tunnel.verified,
        };
    }
    NativeRoutePolicyPayload {
        route: NativeRouteCapability::DirectLan,
        reason: if !tunnel_enabled {
            "tcp_tunnel_operator_disabled"
        } else if !enough_samples {
            "tcp_tunnel_evidence_insufficient"
        } else {
            "tcp_tunnel_success_gate_failed"
        }
        .into(),
        evidence_attempts: tunnel.attempts,
        evidence_verified: tunnel.verified,
    }
}

pub async fn tcp_tunnel_evidence(pool: &PgPool, game_id: &str) -> Result<RouteEvidence, AppError> {
    let row = sqlx::query(
        "WITH room_outcomes AS (
             SELECT room_id,
                    COUNT(*) FILTER (WHERE kind = 'match') AS match_reports,
                    COUNT(*) FILTER (WHERE kind = 'attempt_failure') AS failures,
                    COUNT(DISTINCT payload->'compatibility')
                        FILTER (WHERE kind = 'match') AS compatibility_sets,
                    COUNT(DISTINCT role) FILTER (WHERE kind = 'match') AS role_sets,
                    COUNT(DISTINCT payload->'probe'->>'transcript_checksum')
                        FILTER (WHERE kind = 'match') AS transcript_sets,
                    COUNT(*) FILTER (
                        WHERE kind = 'match'
                          AND payload->'probe'->>'frames_received' = '60'
                          AND payload->'room'->>'state' = 'finished'
                    ) AS complete_reports
             FROM alpha_evidence
             WHERE payload->>'native_route' = 'tcp_tunnel'
               AND payload->'room'->>'game_id' = $1
             GROUP BY room_id
         )
         SELECT COUNT(*) AS attempts,
                COUNT(*) FILTER (
                    WHERE match_reports = 2 AND failures = 0 AND compatibility_sets = 1
                      AND role_sets = 2 AND transcript_sets = 1 AND complete_reports = 2
                ) AS verified
         FROM room_outcomes",
    )
    .bind(game_id)
    .fetch_one(pool)
    .await?;
    Ok(RouteEvidence {
        attempts: u32::try_from(row.try_get::<i64, _>("attempts")?).unwrap_or(u32::MAX),
        verified: u32::try_from(row.try_get::<i64, _>("verified")?).unwrap_or(u32::MAX),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_policy_fails_closed_without_operator_enablement() {
        let decision = decide_native_route(
            false,
            10,
            RouteEvidence {
                attempts: 10,
                verified: 10,
            },
        );
        assert_eq!(decision.route, NativeRouteCapability::DirectLan);
        assert_eq!(decision.reason, "tcp_tunnel_operator_disabled");
    }

    #[test]
    fn route_policy_requires_minimum_sample_and_eighty_percent_success() {
        assert_eq!(
            decide_native_route(
                true,
                10,
                RouteEvidence {
                    attempts: 9,
                    verified: 9
                }
            )
            .route,
            NativeRouteCapability::DirectLan
        );
        assert_eq!(
            decide_native_route(
                true,
                10,
                RouteEvidence {
                    attempts: 10,
                    verified: 7
                }
            )
            .route,
            NativeRouteCapability::DirectLan
        );
        assert_eq!(
            decide_native_route(
                true,
                10,
                RouteEvidence {
                    attempts: 10,
                    verified: 8
                }
            )
            .route,
            NativeRouteCapability::TcpTunnel
        );
    }
}
