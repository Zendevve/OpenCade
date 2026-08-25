use std::collections::HashSet;

use axum::{Json, extract::State, http::StatusCode};
use opencade_protocol::{
    ActivationSummaryPayload, Envelope, ProductEventName, ProductEventPayload, ReadinessBlockCount,
    ReadinessCheckId,
};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::{authn::AuthUser, error::AppError, state::AppState};

const ACTIVATION_WINDOW_DAYS: i32 = 30;
const RAW_RETENTION_DAYS: i32 = 90;
const MIN_AGGREGATE_COHORT: i64 = 3;
const MAX_EVENTS_PER_MINUTE: usize = 60;

pub async fn record_event(
    State(state): State<AppState>,
    user: AuthUser,
    Json(payload): Json<ProductEventPayload>,
) -> Result<(StatusCode, Json<Envelope<Value>>), AppError> {
    if !state
        .auth_rate_limiter
        .check_with_limit(&format!("telemetry:{}", user.id), MAX_EVENTS_PER_MINUTE)
    {
        return Err(AppError::RateLimited(
            "product telemetry rate limit exceeded".into(),
        ));
    }
    let event_id = parse_uuid(&payload.event_id, "event id")?;
    let anonymous_session_id = parse_uuid(&payload.anonymous_session_id, "anonymous session id")?;
    validate_event(&payload)?;

    let known_game =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM games WHERE id = $1)")
            .bind(&payload.game_id)
            .fetch_one(&state.pool)
            .await?;
    if !known_game {
        return Err(AppError::BadRequest("telemetry game id is unknown".into()));
    }

    let blocked_checks = serde_json::to_value(&payload.blocked_checks)?;
    let inserted = sqlx::query(
        "INSERT INTO product_events
            (event_id, anonymous_session_id, event_name, game_id, blocked_checks)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(event_id)
    .bind(anonymous_session_id)
    .bind(event_label(payload.event))
    .bind(&payload.game_id)
    .bind(blocked_checks)
    .execute(&state.pool)
    .await?;

    sqlx::query("DELETE FROM product_events WHERE received_at < now() - make_interval(days => $1)")
        .bind(RAW_RETENTION_DAYS)
        .execute(&state.pool)
        .await?;

    let duplicate = inserted.rows_affected() == 0;
    Ok((
        if duplicate {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        Json(Envelope::new(
            "telemetry.event.accepted",
            json!({ "accepted": true, "duplicate": duplicate }),
        )),
    ))
}

pub async fn activation_summary(
    State(state): State<AppState>,
    _user: AuthUser,
) -> Result<Json<Envelope<ActivationSummaryPayload>>, AppError> {
    let row = sqlx::query(
        "WITH windowed AS (
            SELECT anonymous_session_id, event_name, game_id, received_at
            FROM product_events
            WHERE received_at >= now() - make_interval(days => $1)
         ), selected AS (
            SELECT anonymous_session_id, game_id, MIN(received_at) AS selected_at
            FROM windowed WHERE event_name = 'game_selected'
            GROUP BY anonymous_session_id, game_id
         )
         SELECT
            COUNT(DISTINCT selected.anonymous_session_id) AS selected_sessions,
            COUNT(DISTINCT selected.anonymous_session_id)
                FILTER (WHERE windowed.event_name = 'readiness_completed') AS ready_sessions,
            COUNT(DISTINCT selected.anonymous_session_id)
                FILTER (WHERE windowed.event_name = 'lobby_entered') AS lobby_sessions,
            COUNT(*) FILTER (WHERE windowed.event_name = 'readiness_blocked') AS readiness_blocked_events
         FROM selected
         LEFT JOIN windowed
           ON windowed.anonymous_session_id = selected.anonymous_session_id
          AND windowed.game_id = selected.game_id
          AND windowed.received_at >= selected.selected_at",
    )
    .bind(ACTIVATION_WINDOW_DAYS)
    .fetch_one(&state.pool)
    .await?;

    let selected_sessions: i64 = row.try_get("selected_sessions")?;
    let ready_sessions: i64 = row.try_get("ready_sessions")?;
    let lobby_sessions: i64 = row.try_get("lobby_sessions")?;
    let readiness_blocked_events: i64 = row.try_get("readiness_blocked_events")?;
    if selected_sessions < MIN_AGGREGATE_COHORT {
        return Ok(Json(Envelope::new(
            "telemetry.activation.summary",
            empty_summary(),
        )));
    }
    let blocked_rows = sqlx::query(
        "WITH windowed AS (
            SELECT anonymous_session_id, event_name, game_id, blocked_checks, received_at
            FROM product_events
            WHERE received_at >= now() - make_interval(days => $1)
         ), selected AS (
            SELECT anonymous_session_id, game_id, MIN(received_at) AS selected_at
            FROM windowed WHERE event_name = 'game_selected'
            GROUP BY anonymous_session_id, game_id
         )
         SELECT blocked_check, COUNT(*) AS event_count
         FROM selected
         JOIN windowed
           ON windowed.anonymous_session_id = selected.anonymous_session_id
          AND windowed.game_id = selected.game_id
          AND windowed.received_at >= selected.selected_at
         CROSS JOIN LATERAL jsonb_array_elements_text(windowed.blocked_checks) AS blocked_check
         WHERE windowed.event_name = 'readiness_blocked'
         GROUP BY blocked_check
         HAVING COUNT(*) >= $2
         ORDER BY event_count DESC, blocked_check ASC",
    )
    .bind(ACTIVATION_WINDOW_DAYS)
    .bind(MIN_AGGREGATE_COHORT)
    .fetch_all(&state.pool)
    .await?;
    let blocked_by_check = blocked_rows
        .into_iter()
        .map(|row| {
            let label: String = row.try_get("blocked_check")?;
            Ok(ReadinessBlockCount {
                check: parse_check_label(&label)?,
                count: row.try_get("event_count")?,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(Envelope::new(
        "telemetry.activation.summary",
        ActivationSummaryPayload {
            window_days: ACTIVATION_WINDOW_DAYS as u16,
            selected_sessions,
            ready_sessions,
            lobby_sessions,
            readiness_blocked_events,
            selected_to_ready_rate: ratio(ready_sessions, selected_sessions),
            selected_to_lobby_rate: ratio(lobby_sessions, selected_sessions),
            blocked_by_check,
        },
    )))
}

fn validate_event(payload: &ProductEventPayload) -> Result<(), AppError> {
    if payload.game_id.len() < 3
        || payload.game_id.len() > 20
        || !payload
            .game_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(AppError::BadRequest("telemetry game id is invalid".into()));
    }
    let unique = payload
        .blocked_checks
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if unique.len() != payload.blocked_checks.len() {
        return Err(AppError::BadRequest(
            "telemetry blocked checks must be unique".into(),
        ));
    }
    match payload.event {
        ProductEventName::ReadinessBlocked => {
            if unique.is_empty() || unique.contains(&ReadinessCheckId::Network) || unique.len() > 4
            {
                return Err(AppError::BadRequest(
                    "readiness_blocked requires one or more blocking checks".into(),
                ));
            }
        }
        _ if !unique.is_empty() => {
            return Err(AppError::BadRequest(
                "blocked checks are only valid for readiness_blocked".into(),
            ));
        }
        _ => {}
    }
    Ok(())
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value)
        .map_err(|_| AppError::BadRequest(format!("telemetry {field} is invalid")))
}

fn event_label(event: ProductEventName) -> &'static str {
    match event {
        ProductEventName::GameSelected => "game_selected",
        ProductEventName::ReadinessCompleted => "readiness_completed",
        ProductEventName::ReadinessBlocked => "readiness_blocked",
        ProductEventName::LobbyEntered => "lobby_entered",
    }
}

fn parse_check_label(label: &str) -> Result<ReadinessCheckId, AppError> {
    match label {
        "desktop" => Ok(ReadinessCheckId::Desktop),
        "control_plane" => Ok(ReadinessCheckId::ControlPlane),
        "game_runtime" => Ok(ReadinessCheckId::GameRuntime),
        "native_port" => Ok(ReadinessCheckId::NativePort),
        "network" => Ok(ReadinessCheckId::Network),
        _ => Err(AppError::Internal("invalid telemetry aggregate".into())),
    }
}

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn empty_summary() -> ActivationSummaryPayload {
    ActivationSummaryPayload {
        window_days: ACTIVATION_WINDOW_DAYS as u16,
        selected_sessions: 0,
        ready_sessions: 0,
        lobby_sessions: 0,
        readiness_blocked_events: 0,
        selected_to_ready_rate: 0.0,
        selected_to_lobby_rate: 0.0,
        blocked_by_check: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        event: ProductEventName,
        blocked_checks: Vec<ReadinessCheckId>,
    ) -> ProductEventPayload {
        ProductEventPayload {
            event_id: Uuid::new_v4().to_string(),
            anonymous_session_id: Uuid::new_v4().to_string(),
            event,
            game_id: "sfiii3".into(),
            blocked_checks,
        }
    }

    #[test]
    fn closed_contract_rejects_invalid_blocker_combinations() {
        assert!(validate_event(&event(ProductEventName::ReadinessBlocked, vec![])).is_err());
        assert!(
            validate_event(&event(
                ProductEventName::ReadinessBlocked,
                vec![ReadinessCheckId::Network]
            ))
            .is_err()
        );
        assert!(
            validate_event(&event(
                ProductEventName::ReadinessCompleted,
                vec![ReadinessCheckId::Desktop]
            ))
            .is_err()
        );
        assert!(
            validate_event(&event(
                ProductEventName::ReadinessBlocked,
                vec![ReadinessCheckId::Desktop, ReadinessCheckId::Desktop]
            ))
            .is_err()
        );
        assert!(
            validate_event(&event(
                ProductEventName::ReadinessBlocked,
                vec![ReadinessCheckId::Desktop]
            ))
            .is_ok()
        );
    }

    #[test]
    fn empty_funnel_has_zero_rates() {
        assert_eq!(ratio(0, 0), 0.0);
        assert_eq!(ratio(3, 4), 0.75);
    }
}
