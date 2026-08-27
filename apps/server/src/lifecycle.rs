use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

/// Starts the authoritative deadline reconciler. Database row locks make the
/// operation safe when several server replicas run it concurrently.
pub fn spawn_reconciler(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = reconcile_expired(&state).await {
                tracing::error!(%error, "match lifecycle reconciliation failed");
            }
        }
    });
}

pub async fn reconcile_expired(state: &AppState) -> Result<u64, AppError> {
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "UPDATE challenges SET state = 'EXPIRED'
         WHERE state = 'PENDING' AND expires_at <= now()",
    )
    .execute(&mut *transaction)
    .await?;

    let room_ids = sqlx::query_scalar::<_, Uuid>(
        "WITH expired AS MATERIALIZED (
             SELECT id, attempt_id FROM rooms
             WHERE state IN ('WAITING', 'CHALLENGING', 'CONNECTING', 'PLAYING')
               AND state_deadline_at <= now()
             ORDER BY state_deadline_at
             FOR UPDATE SKIP LOCKED
         ), updated_rooms AS (
             UPDATE rooms AS room
             SET state = 'CANCELLED', state_deadline_at = NULL
             FROM expired WHERE room.id = expired.id
             RETURNING room.id, room.attempt_id
         ), updated_attempts AS (
             UPDATE match_attempts AS attempt
             SET state = 'EXPIRED', failure_code = 'state_deadline_exceeded',
                 deadline_at = NULL, finished_at = now()
             FROM expired
             WHERE attempt.attempt_id = expired.attempt_id AND attempt.state = 'ACTIVE'
         ), updated_challenges AS (
             UPDATE challenges AS challenge SET state = 'EXPIRED'
             FROM expired
             WHERE challenge.room_id = expired.id AND challenge.state = 'PENDING'
         )
         INSERT INTO room_events (room_id, attempt_id, event_type, payload)
         SELECT id, attempt_id, 'room.deadline.expired', $1 FROM updated_rooms
         RETURNING room_id",
    )
    .bind(json!({ "failure_code": "state_deadline_exceeded" }))
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;

    let members = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT room_id, user_id FROM room_members WHERE room_id = ANY($1)",
    )
    .bind(&room_ids)
    .fetch_all(&state.pool)
    .await?;
    for (room_id, member) in members {
        state.notify_user(
            member,
            "room.deadline.expired",
            json!({ "room_id": room_id, "state": "cancelled" }),
        );
    }
    Ok(room_ids.len() as u64)
}
