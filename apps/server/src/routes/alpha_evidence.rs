use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use opencade_networking::summarize_campaign_evidence;
use opencade_protocol::{AlphaFailureReport, Envelope, MatchReport, MatchReportRole};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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
    let expected_role = member_role(&state, room_id, user.id).await?;
    if role != expected_role {
        return Err(AppError::Forbidden(
            "evidence role does not match room membership".into(),
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

async fn member_role(
    state: &AppState,
    room_id: Uuid,
    user_id: Uuid,
) -> Result<MatchReportRole, AppError> {
    let host = sqlx::query_scalar::<_, Uuid>(
        "SELECT rooms.host_user_id FROM rooms
         JOIN room_members ON room_members.room_id = rooms.id
         WHERE rooms.id = $1 AND room_members.user_id = $2",
    )
    .bind(room_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Forbidden("not a room member".into()))?;
    Ok(if host == user_id {
        MatchReportRole::Host
    } else {
        MatchReportRole::Guest
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
