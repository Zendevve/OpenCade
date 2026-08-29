use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use opencade_networking::summarize_campaign_evidence;
use opencade_protocol::{
    AlphaFailureReport, Envelope, MatchReport, MatchReportRole, NativeRouteCapability,
    PublicCompatibilityCohort, PublicCompatibilityPayload,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::{authn::AuthUser, error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRequest {
    evidence: Value,
}

pub async fn submit_evidence(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<EvidenceRequest>,
) -> Result<(StatusCode, Json<Envelope<Value>>), AppError> {
    let raw_encoded = serde_json::to_vec(&request.evidence)
        .map_err(|_| AppError::BadRequest("evidence is invalid".into()))?;
    if raw_encoded.len() > 64 * 1024 {
        return Err(AppError::BadRequest(
            "evidence exceeds the 64 KiB limit".into(),
        ));
    }
    let (room_id, role, kind, canonical) = decode_evidence(&request.evidence)?;
    if contains_private_key(&canonical) {
        return Err(AppError::BadRequest(
            "evidence contains private fields".into(),
        ));
    }
    let encoded = serde_json::to_vec(&canonical)
        .map_err(|_| AppError::BadRequest("evidence is invalid".into()))?;
    let context = evidence_room_context(&state, room_id, user.id).await?;
    if role != context.role {
        return Err(AppError::Forbidden(
            "evidence role does not match room membership".into(),
        ));
    }
    if canonical
        .get("room")
        .and_then(|room| room.get("game_id"))
        .and_then(Value::as_str)
        != Some(context.game_id.as_str())
    {
        return Err(AppError::BadRequest(
            "evidence game does not match the authoritative room".into(),
        ));
    }
    if kind == "match" && context.state != "FINISHED" {
        return Err(AppError::Conflict(
            "successful match evidence requires an authoritative finished room".into(),
        ));
    }
    let digest = hex::encode(Sha256::digest(&encoded));
    let inserted = sqlx::query(
        "INSERT INTO alpha_evidence (digest, room_id, submitted_by, role, kind, payload)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT DO NOTHING",
    )
    .bind(&digest)
    .bind(room_id)
    .bind(user.id)
    .bind(role_label(role))
    .bind(kind)
    .bind(canonical)
    .execute(&state.pool)
    .await?;
    reject_conflicting_duplicate(&state, room_id, user.id, kind, &digest, &inserted).await?;
    let duplicate = inserted.rows_affected() == 0;
    Ok((
        if duplicate {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        Json(Envelope::new(
            "alpha.evidence.accepted",
            json!({ "digest": digest, "duplicate": duplicate }),
        )),
    ))
}

pub async fn campaign_summary(
    State(state): State<AppState>,
    _user: AuthUser,
    headers: HeaderMap,
) -> Result<Json<Envelope<Value>>, AppError> {
    state.require_operator(
        headers
            .get("x-operator-token")
            .and_then(|value| value.to_str().ok()),
    )?;
    let values = sqlx::query_scalar::<_, Value>(
        "WITH recent_rooms AS (
             SELECT room_id, MAX(created_at) AS latest
             FROM alpha_evidence GROUP BY room_id
             ORDER BY latest DESC LIMIT 500
         )
         SELECT evidence.payload FROM alpha_evidence AS evidence
         JOIN recent_rooms USING (room_id)
         ORDER BY evidence.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    let mut matches = Vec::new();
    let mut failures = Vec::new();
    for value in values {
        if value.get("kind").is_none() {
            if let Ok(report) = serde_json::from_value::<MatchReport>(value) {
                matches.push(report);
            }
        } else if let Ok(report) = serde_json::from_value::<AlphaFailureReport>(value) {
            failures.push(report);
        }
    }
    let total_rooms =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(DISTINCT room_id) FROM alpha_evidence")
            .fetch_one(&state.pool)
            .await?;
    let mut summary = serde_json::to_value(summarize_campaign_evidence(&matches, &failures))?;
    if let Some(summary) = summary.as_object_mut() {
        summary.insert("cohort_limit".into(), json!(500));
        summary.insert("total_evidence_rooms".into(), json!(total_rooms));
        summary.insert("cohort_truncated".into(), json!(total_rooms > 500));
    }
    Ok(Json(Envelope::new("alpha.campaign.summary", summary)))
}

pub async fn public_compatibility(
    State(state): State<AppState>,
) -> Result<Json<Envelope<Value>>, AppError> {
    const MINIMUM_COHORT_SIZE: i64 = 3;
    const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);
    let mut cache = state.public_compatibility_cache.write().await;
    if let Some((created_at, payload)) = cache.as_ref()
        && created_at.elapsed() < CACHE_TTL
    {
        return Ok(Json(Envelope::new(
            "public.compatibility.summary",
            json!(payload),
        )));
    }
    let rows = sqlx::query(
        "WITH room_status AS (
             SELECT room_id,
                    COUNT(*) FILTER (WHERE kind = 'match') AS match_reports,
                    COUNT(*) FILTER (WHERE kind = 'attempt_failure') AS failures,
                    COUNT(DISTINCT payload->'compatibility')
                        FILTER (WHERE kind = 'match') AS compatibility_sets,
                    COUNT(DISTINCT payload->>'native_route')
                        FILTER (WHERE kind = 'match') AS route_sets,
                    COUNT(DISTINCT role) FILTER (WHERE kind = 'match') AS role_sets,
                    COUNT(DISTINCT payload->'probe'->>'transcript_checksum')
                        FILTER (WHERE kind = 'match') AS transcript_sets,
                    COUNT(*) FILTER (
                        WHERE kind = 'match'
                          AND payload->'probe'->>'frames_received' = '60'
                          AND payload->'room'->>'state' = 'finished'
                    ) AS complete_reports
             FROM alpha_evidence
             GROUP BY room_id
         ), samples AS (
             SELECT DISTINCT evidence.room_id,
                    evidence.payload->'room'->>'game_id' AS game_id,
                    evidence.payload->'client'->>'platform' AS platform,
                    evidence.payload->'compatibility'->>'adapter' AS adapter,
                    evidence.payload->'compatibility'->>'emulator_version' AS emulator_version,
                    evidence.payload->'probe'->>'transport' AS transport,
                    evidence.payload->>'native_route' AS native_route,
                    status.match_reports = 2 AND status.failures = 0
                        AND status.compatibility_sets = 1 AND status.route_sets = 1
                        AND status.role_sets = 2 AND status.transcript_sets = 1
                        AND status.complete_reports = 2 AS verified
             FROM alpha_evidence AS evidence
             JOIN room_status AS status USING (room_id)
             WHERE evidence.kind = 'match'
               AND evidence.payload ? 'compatibility'
               AND evidence.payload ? 'native_route'
         )
         SELECT game_id, platform, adapter, emulator_version, transport, native_route,
                COUNT(DISTINCT room_id) AS attempts,
                COUNT(DISTINCT room_id) FILTER (WHERE verified) AS verified
         FROM samples
         WHERE game_id IS NOT NULL AND platform IS NOT NULL AND adapter IS NOT NULL
           AND transport IS NOT NULL AND native_route IN ('direct_lan', 'tcp_tunnel')
         GROUP BY game_id, platform, adapter, emulator_version, transport, native_route
         HAVING COUNT(DISTINCT room_id) >= $1
         ORDER BY game_id, platform, adapter, emulator_version, transport, native_route
         LIMIT 500",
    )
    .bind(MINIMUM_COHORT_SIZE)
    .fetch_all(&state.pool)
    .await?;
    let cohorts = rows
        .into_iter()
        .map(|row| -> Result<PublicCompatibilityCohort, AppError> {
            let route: String = row.try_get("native_route")?;
            Ok(PublicCompatibilityCohort {
                game_id: row.try_get("game_id")?,
                platform: row.try_get("platform")?,
                adapter: row.try_get("adapter")?,
                emulator_version: row.try_get("emulator_version")?,
                transport: row.try_get("transport")?,
                native_route: match route.as_str() {
                    "direct_lan" => NativeRouteCapability::DirectLan,
                    "tcp_tunnel" => NativeRouteCapability::TcpTunnel,
                    _ => return Err(AppError::Internal("invalid native route aggregate".into())),
                },
                attempts: u32::try_from(row.try_get::<i64, _>("attempts")?).unwrap_or(u32::MAX),
                verified: u32::try_from(row.try_get::<i64, _>("verified")?).unwrap_or(u32::MAX),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let payload = PublicCompatibilityPayload {
        schema_version: 1,
        generated_at: chrono::Utc::now(),
        minimum_cohort_size: u8::try_from(MINIMUM_COHORT_SIZE).unwrap_or(u8::MAX),
        cohorts,
    };
    *cache = Some((std::time::Instant::now(), payload.clone()));
    Ok(Json(Envelope::new(
        "public.compatibility.summary",
        json!(payload),
    )))
}

fn decode_evidence(
    value: &Value,
) -> Result<(Uuid, MatchReportRole, &'static str, Value), AppError> {
    match value.get("kind") {
        None => {
            let report: MatchReport = serde_json::from_value(value.clone())
                .map_err(|_| AppError::BadRequest("match evidence is invalid".into()))?;
            let room_id = parse_room_id(&report.room.id)?;
            let role = report.probe.role;
            let canonical = serde_json::to_value(report)
                .map_err(|_| AppError::BadRequest("match evidence is invalid".into()))?;
            Ok((room_id, role, "match", canonical))
        }
        Some(Value::String(kind)) if kind == "attempt_failure" => {
            let report: AlphaFailureReport = serde_json::from_value(value.clone())
                .map_err(|_| AppError::BadRequest("failure evidence is invalid".into()))?;
            let room_id = parse_room_id(&report.room.id)?;
            let role = report.role;
            let canonical = serde_json::to_value(report)
                .map_err(|_| AppError::BadRequest("failure evidence is invalid".into()))?;
            Ok((room_id, role, "attempt_failure", canonical))
        }
        _ => Err(AppError::BadRequest("evidence kind is invalid".into())),
    }
}

async fn reject_conflicting_duplicate(
    state: &AppState,
    room_id: Uuid,
    user_id: Uuid,
    kind: &str,
    digest: &str,
    inserted: &sqlx::postgres::PgQueryResult,
) -> Result<(), AppError> {
    if inserted.rows_affected() == 1 {
        return Ok(());
    }
    let existing = sqlx::query_scalar::<_, String>(
        "SELECT digest FROM alpha_evidence
         WHERE room_id = $1 AND submitted_by = $2 AND kind = $3",
    )
    .bind(room_id)
    .bind(user_id)
    .bind(kind)
    .fetch_optional(&state.pool)
    .await?;
    if existing.as_deref().is_some_and(|value| value != digest) {
        return Err(AppError::Conflict(
            "different evidence already exists for this attempt and role".into(),
        ));
    }
    Ok(())
}

struct EvidenceRoomContext {
    role: MatchReportRole,
    game_id: String,
    state: String,
}

async fn evidence_room_context(
    state: &AppState,
    room_id: Uuid,
    user_id: Uuid,
) -> Result<EvidenceRoomContext, AppError> {
    let row = sqlx::query(
        "SELECT rooms.host_user_id, rooms.game_id, rooms.state FROM rooms
         JOIN room_members ON room_members.room_id = rooms.id
         WHERE rooms.id = $1 AND room_members.user_id = $2",
    )
    .bind(room_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Forbidden("not a room member".into()))?;
    let host: Uuid = row.try_get("host_user_id")?;
    Ok(EvidenceRoomContext {
        role: if host == user_id {
            MatchReportRole::Host
        } else {
            MatchReportRole::Guest
        },
        game_id: row.try_get("game_id")?,
        state: row.try_get("state")?,
    })
}

fn contains_private_key(value: &Value) -> bool {
    const FORBIDDEN: &[&str] = &[
        "token",
        "session_token",
        "local_user_id",
        "peer_user_id",
        "host_id",
        "guest_id",
        "endpoint",
        "nonce",
        "path",
        "rom_path",
    ];
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, value)| FORBIDDEN.contains(&key.as_str()) || contains_private_key(value)),
        Value::Array(values) => values.iter().any(contains_private_key),
        _ => false,
    }
}

fn parse_room_id(value: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value).map_err(|_| AppError::BadRequest("evidence room id is invalid".into()))
}

fn role_label(role: MatchReportRole) -> &'static str {
    match role {
        MatchReportRole::Host => "host",
        MatchReportRole::Guest => "guest",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_filter_walks_nested_evidence() {
        assert!(!contains_private_key(&json!({"room": {"id": "safe"}})));
        assert!(contains_private_key(
            &json!({"probe": {"endpoint": "127.0.0.1"}})
        ));
        assert!(contains_private_key(&json!([{"session_token": "secret"}])));
    }
}
