-- Authoritative match-attempt identity, deadlines, and lifecycle audit metadata.

ALTER TABLE product_events DROP CONSTRAINT IF EXISTS product_events_event_name_check;
ALTER TABLE product_events ADD CONSTRAINT product_events_event_name_check CHECK (
    event_name IN (
        'game_selected', 'readiness_completed', 'readiness_blocked', 'lobby_entered',
        'launch_attempted', 'launch_succeeded'
    )
);

ALTER TABLE rooms
    ADD COLUMN IF NOT EXISTS attempt_id UUID NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN IF NOT EXISTS state_deadline_at TIMESTAMPTZ;

UPDATE rooms
SET state_deadline_at = CASE state
    WHEN 'WAITING' THEN updated_at + interval '24 hours'
    WHEN 'CHALLENGING' THEN updated_at + interval '5 minutes'
    WHEN 'CONNECTING' THEN updated_at + interval '2 minutes'
    ELSE NULL
END
WHERE state_deadline_at IS NULL;

ALTER TABLE challenges
    ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ NOT NULL
        DEFAULT (now() + interval '5 minutes');

ALTER TABLE challenges DROP CONSTRAINT IF EXISTS challenges_state_check;
ALTER TABLE challenges ADD CONSTRAINT challenges_state_check
    CHECK (state IN ('PENDING','ACCEPTED','DECLINED','CANCELLED','EXPIRED'));

CREATE UNIQUE INDEX IF NOT EXISTS uq_challenges_active_pair
    ON challenges (challenger_id, challenged_id)
    WHERE state = 'PENDING';
CREATE INDEX IF NOT EXISTS idx_challenges_expiry
    ON challenges (expires_at) WHERE state = 'PENDING';

ALTER TABLE match_preflights ADD COLUMN IF NOT EXISTS attempt_id UUID;
UPDATE match_preflights AS preflight
SET attempt_id = rooms.attempt_id
FROM rooms
WHERE rooms.id = preflight.room_id AND preflight.attempt_id IS NULL;
ALTER TABLE match_preflights ALTER COLUMN attempt_id SET NOT NULL;

ALTER TABLE room_launch_barriers ADD COLUMN IF NOT EXISTS attempt_id UUID;
UPDATE room_launch_barriers AS barrier
SET attempt_id = rooms.attempt_id
FROM rooms
WHERE rooms.id = barrier.room_id AND barrier.attempt_id IS NULL;
ALTER TABLE room_launch_barriers ALTER COLUMN attempt_id SET NOT NULL;

ALTER TABLE room_events
    ADD COLUMN IF NOT EXISTS schema_version SMALLINT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS attempt_id UUID,
    ADD COLUMN IF NOT EXISTS actor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS causation_id UUID,
    ADD COLUMN IF NOT EXISTS payload JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE TABLE IF NOT EXISTS match_attempts (
    attempt_id UUID PRIMARY KEY,
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK (state IN ('ACTIVE','SUCCEEDED','FAILED','CANCELLED','EXPIRED')),
    failure_code TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deadline_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    UNIQUE (room_id, attempt_id)
);

INSERT INTO match_attempts (attempt_id, room_id, state, deadline_at, finished_at)
SELECT attempt_id, id,
       CASE state
           WHEN 'FINISHED' THEN 'SUCCEEDED'
           WHEN 'CANCELLED' THEN 'CANCELLED'
           ELSE 'ACTIVE'
       END,
       state_deadline_at,
       CASE WHEN state IN ('FINISHED', 'CANCELLED') THEN updated_at ELSE NULL END
FROM rooms
ON CONFLICT (attempt_id) DO NOTHING;

CREATE INDEX IF NOT EXISTS idx_match_attempts_room_started
    ON match_attempts (room_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_match_attempts_deadline
    ON match_attempts (deadline_at) WHERE state = 'ACTIVE';

ALTER TABLE match_preflights
    DROP CONSTRAINT IF EXISTS match_preflights_attempt_id_fkey,
    ADD CONSTRAINT match_preflights_attempt_id_fkey
        FOREIGN KEY (attempt_id) REFERENCES match_attempts(attempt_id) ON DELETE CASCADE;
ALTER TABLE room_launch_barriers
    DROP CONSTRAINT IF EXISTS room_launch_barriers_attempt_id_fkey,
    ADD CONSTRAINT room_launch_barriers_attempt_id_fkey
        FOREIGN KEY (attempt_id) REFERENCES match_attempts(attempt_id) ON DELETE CASCADE;
ALTER TABLE room_events
    DROP CONSTRAINT IF EXISTS room_events_attempt_id_fkey,
    ADD CONSTRAINT room_events_attempt_id_fkey
        FOREIGN KEY (attempt_id) REFERENCES match_attempts(attempt_id) ON DELETE RESTRICT;
CREATE INDEX IF NOT EXISTS idx_room_events_attempt_revision
    ON room_events (attempt_id, revision) WHERE attempt_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_rooms_waiting_host_game
    ON rooms (host_user_id, game_id) WHERE state = 'WAITING';

CREATE TABLE IF NOT EXISTS command_results (
    actor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    idempotency_key UUID NOT NULL,
    command_type TEXT NOT NULL,
    response_type TEXT NOT NULL,
    response_payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (actor_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_command_results_created_at
    ON command_results (created_at);
