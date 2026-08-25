use axum::{Json, extract::Path, extract::State, http::StatusCode};
use chrono::{DateTime, Duration, Utc};
use opencade_protocol::{
    Envelope, LaunchBarrierPayload, MatchPreflightPayload, MatchReportCompatibility,
    NativeRouteCapability, RoomInvitePayload, RoomSnapshotPayload,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    authn::{AuthUser, hash_token},
    error::AppError,
    room_state::{RoomEvent, from_database, to_database, transition},
    state::AppState,
};

use super::rooms::{notify_room_state, room_payload};

const INVITE_TTL_MINUTES: i64 = 15;
const LAUNCH_DELAY_SECONDS: i64 = 5;

#[derive(Debug, Deserialize)]
pub struct JoinInviteRequest {
    code: String,
}

pub async fn create_invite(
    State(state): State<AppState>,
    user: AuthUser,
    Path(room_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Envelope<Value>>), AppError> {
    let host = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM rooms WHERE id = $1 AND host_user_id = $2 AND state = 'WAITING')",
    )
    .bind(room_id)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    if !host {
        return Err(AppError::Forbidden(
            "only the host of a waiting room can create an invite".into(),
        ));
    }

    let code = Uuid::new_v4().simple().to_string()[..10].to_ascii_uppercase();
    let expires_at = Utc::now() + Duration::minutes(INVITE_TTL_MINUTES);
    sqlx::query(
        "INSERT INTO room_invites (code_hash, room_id, created_by, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(hash_token(&code))
    .bind(room_id)
    .bind(user.id)
    .bind(expires_at)
    .execute(&state.pool)
    .await?;
    sqlx::query("INSERT INTO room_events (room_id, event_type) VALUES ($1, 'room.invite.created')")
        .bind(room_id)
        .execute(&state.pool)
        .await?;

    let payload = RoomInvitePayload {
        room_id: room_id.to_string(),
        code,
        expires_at,
    };
    Ok((
        StatusCode::CREATED,
        Json(Envelope::new("room.invite.created", json!(payload))),
    ))
}

pub async fn join_invite(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<JoinInviteRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let code = request.code.trim().to_ascii_uppercase();
    if code.len() != 10 || !code.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::BadRequest("invite code is invalid".into()));
    }

    let mut transaction = state.pool.begin().await?;
    let row = sqlx::query(
        "SELECT room_id FROM room_invites
         WHERE code_hash = $1 AND expires_at > now() AND consumed_at IS NULL
         FOR UPDATE",
    )
    .bind(hash_token(&code))
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("invite is invalid, expired, or already used".into()))?;
    let room_id: Uuid = row
        .try_get("room_id")
        .map_err(|_| AppError::Internal("invalid invite record".into()))?;
    let room =
        sqlx::query("SELECT host_user_id, game_id, state FROM rooms WHERE id = $1 FOR UPDATE")
            .bind(room_id)
            .fetch_one(&mut *transaction)
            .await?;
    let host_id: Uuid = room
        .try_get("host_user_id")
        .map_err(|_| AppError::Internal("invalid room record".into()))?;
    let game_id: String = room
        .try_get("game_id")
        .map_err(|_| AppError::Internal("invalid room record".into()))?;
    if host_id == user.id {
        return Err(AppError::Forbidden(
            "room hosts cannot redeem their own invite".into(),
        ));
    }
    let current = from_database(
        room.try_get::<String, _>("state")
            .map_err(|_| AppError::Internal("invalid room record".into()))?
            .as_str(),
    )
    .map_err(AppError::Internal)?;
    let next = transition(current, RoomEvent::Accept)
        .map_err(|error| AppError::Conflict(error.to_string()))?;
    let members =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM room_members WHERE room_id = $1")
            .bind(room_id)
            .fetch_one(&mut *transaction)
            .await?;
    if members >= 2 {
        return Err(AppError::Conflict("room is full".into()));
    }
    sqlx::query(
        "UPDATE rooms SET state = 'CANCELLED'
         WHERE host_user_id = $1 AND game_id = $2 AND state = 'WAITING' AND id <> $3",
    )
    .bind(user.id)
    .bind(&game_id)
    .bind(room_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("INSERT INTO room_members (room_id, user_id) VALUES ($1, $2)")
        .bind(room_id)
        .bind(user.id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("UPDATE rooms SET state = $2 WHERE id = $1")
        .bind(room_id)
        .bind(to_database(&next))
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "UPDATE room_invites SET consumed_at = now(), consumed_by = $2 WHERE code_hash = $1",
    )
    .bind(hash_token(&code))
    .bind(user.id)
    .execute(&mut *transaction)
    .await?;
    record_event(&mut transaction, room_id, "room.invite.joined").await?;
    transaction.commit().await?;

    notify_room_state(&state, room_id, user.id).await
}

pub async fn submit_preflight(
    State(state): State<AppState>,
    user: AuthUser,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<MatchPreflightPayload>,
) -> Result<Json<Envelope<Value>>, AppError> {
    if payload.room_id != room_id.to_string() {
        return Err(AppError::BadRequest(
            "preflight room does not match path".into(),
        ));
    }
    validate_compatibility(&payload.compatibility)?;
    ensure_member(&state, room_id, user.id).await?;
    sqlx::query(
        "INSERT INTO match_preflights
            (room_id, user_id, compatibility, native_port_available, ready, updated_at)
         VALUES ($1, $2, $3, $4, FALSE, now())
         ON CONFLICT (room_id, user_id) DO UPDATE SET
            compatibility = EXCLUDED.compatibility,
            native_port_available = EXCLUDED.native_port_available,
            ready = FALSE,
            updated_at = now()",
    )
    .bind(room_id)
    .bind(user.id)
    .bind(json!(payload.compatibility))
    .bind(payload.native_port_available)
    .execute(&state.pool)
    .await?;
    sqlx::query("INSERT INTO room_events (room_id, event_type) VALUES ($1, 'match.preflight')")
        .bind(room_id)
        .execute(&state.pool)
        .await?;
    let snapshot = build_snapshot(&state, room_id, user.id).await?;
    notify_members(&state, room_id, "room.snapshot", json!(snapshot)).await?;
    Ok(Json(Envelope::new(
        "match.preflight.accepted",
        json!(snapshot),
    )))
}

pub async fn room_snapshot(
    State(state): State<AppState>,
    user: AuthUser,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let snapshot = build_snapshot(&state, room_id, user.id).await?;
    Ok(Json(Envelope::new("room.snapshot", json!(snapshot))))
}

pub async fn ready_to_launch(
    State(state): State<AppState>,
    user: AuthUser,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    ensure_member(&state, room_id, user.id).await?;
    let snapshot = build_snapshot(&state, room_id, user.id).await?;
    if snapshot.preflight_count != 2 || !snapshot.compatibility_matched {
        return Err(AppError::Conflict(
            "both peers must pass matching compatibility preflight".into(),
        ));
    }
    sqlx::query(
        "UPDATE match_preflights SET ready = TRUE, updated_at = now()
         WHERE room_id = $1 AND user_id = $2 AND native_port_available = TRUE",
    )
    .bind(room_id)
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    let ready_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM match_preflights WHERE room_id = $1 AND ready = TRUE",
    )
    .bind(room_id)
    .fetch_one(&state.pool)
    .await?;
    if ready_count == 2 {
        sqlx::query(
            "INSERT INTO room_launch_barriers (room_id, launch_at)
             VALUES ($1, now() + make_interval(secs => $2))
             ON CONFLICT (room_id) DO NOTHING",
        )
        .bind(room_id)
        .bind(LAUNCH_DELAY_SECONDS as f64)
        .execute(&state.pool)
        .await?;
    }
    sqlx::query("INSERT INTO room_events (room_id, event_type) VALUES ($1, 'match.launch.ready')")
        .bind(room_id)
        .execute(&state.pool)
        .await?;
    let snapshot = build_snapshot(&state, room_id, user.id).await?;
    notify_members(&state, room_id, "room.snapshot", json!(snapshot)).await?;
    Ok(Json(Envelope::new("match.launch.ready", json!(snapshot))))
}

async fn build_snapshot(
    state: &AppState,
    room_id: Uuid,
    user_id: Uuid,
) -> Result<RoomSnapshotPayload, AppError> {
    let room = room_payload(state, room_id, user_id).await?;
    let rows = sqlx::query(
        "SELECT compatibility, native_port_available, ready
         FROM match_preflights WHERE room_id = $1 ORDER BY user_id",
    )
    .bind(room_id)
    .fetch_all(&state.pool)
    .await?;
    let compatibilities = rows
        .iter()
        .filter_map(|row| row.try_get::<Value, _>("compatibility").ok())
        .collect::<Vec<_>>();
    let compatibility_matched = compatibilities.len() == 2
        && compatibilities[0] == compatibilities[1]
        && rows.iter().all(|row| {
            row.try_get::<bool, _>("native_port_available")
                .unwrap_or(false)
        });
    let ready_count = rows
        .iter()
        .filter(|row| row.try_get::<bool, _>("ready").unwrap_or(false))
        .count();
    let launch_at = sqlx::query_scalar::<_, DateTime<Utc>>(
        "SELECT launch_at FROM room_launch_barriers WHERE room_id = $1",
    )
    .bind(room_id)
    .fetch_optional(&state.pool)
    .await?;
    let revision = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(max(revision), 0) FROM room_events WHERE room_id = $1",
    )
    .bind(room_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(RoomSnapshotPayload {
        room,
        revision,
        preflight_count: u8::try_from(rows.len()).unwrap_or(u8::MAX),
        compatibility_matched,
        barrier: LaunchBarrierPayload {
            room_id: room_id.to_string(),
            ready_count: u8::try_from(ready_count).unwrap_or(u8::MAX),
            required_count: 2,
            launch_at,
        },
        route: NativeRouteCapability::DirectLan,
    })
}

async fn ensure_member(state: &AppState, room_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let member = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM room_members WHERE room_id = $1 AND user_id = $2)",
    )
    .bind(room_id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;
    if member {
        Ok(())
    } else {
        Err(AppError::Forbidden("not a room member".into()))
    }
}

async fn notify_members(
    state: &AppState,
    room_id: Uuid,
    event: &str,
    payload: Value,
) -> Result<(), AppError> {
    let members =
        sqlx::query_scalar::<_, Uuid>("SELECT user_id FROM room_members WHERE room_id = $1")
            .bind(room_id)
            .fetch_all(&state.pool)
            .await?;
    for member in members {
        state.notify_user(member, event, payload.clone());
    }
    Ok(())
}

async fn record_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    room_id: Uuid,
    event: &str,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO room_events (room_id, event_type) VALUES ($1, $2)")
        .bind(room_id)
        .bind(event)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

fn validate_compatibility(value: &MatchReportCompatibility) -> Result<(), AppError> {
    let valid_hash = |hash: &str| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    if value.adapter != "retroarch_fbneo"
        || value
            .emulator_version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty() || version.len() > 64)
        || !valid_hash(&value.executable_sha256)
        || !valid_hash(&value.core_sha256)
        || !valid_hash(&value.content_sha256)
    {
        return Err(AppError::BadRequest(
            "compatibility preflight is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compatibility() -> MatchReportCompatibility {
        MatchReportCompatibility {
            adapter: "retroarch_fbneo".into(),
            emulator_version: Some("1.22.0".into()),
            executable_sha256: "a".repeat(64),
            core_sha256: "b".repeat(64),
            content_sha256: "c".repeat(64),
        }
    }

    #[test]
    fn compatibility_is_fail_closed() {
        assert!(validate_compatibility(&compatibility()).is_ok());
        let mut invalid = compatibility();
        invalid.content_sha256 = "C:\\ROMs\\sfiii3.zip".into();
        assert!(validate_compatibility(&invalid).is_err());
    }
}
