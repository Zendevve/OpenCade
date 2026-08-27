-- Community alpha orchestration, compatibility handshakes, and privacy-safe evidence.

CREATE TABLE room_invites (
    code_hash TEXT PRIMARY KEY,
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    consumed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_room_invites_room ON room_invites(room_id, expires_at);

CREATE TABLE match_preflights (
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    compatibility JSONB NOT NULL,
    native_port_available BOOLEAN NOT NULL,
    ready BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (room_id, user_id)
);

CREATE TABLE room_launch_barriers (
    room_id UUID PRIMARY KEY REFERENCES rooms(id) ON DELETE CASCADE,
    launch_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE room_events (
    revision BIGSERIAL PRIMARY KEY,
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT room_events_type CHECK (event_type ~ '^[a-z0-9_.]{3,64}$')
);

CREATE INDEX idx_room_events_room_revision ON room_events(room_id, revision DESC);

CREATE TABLE alpha_evidence (
    digest TEXT PRIMARY KEY,
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    submitted_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('host', 'guest')),
    kind TEXT NOT NULL CHECK (kind IN ('match', 'attempt_failure')),
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (room_id, submitted_by, kind)
);

CREATE INDEX idx_alpha_evidence_room ON alpha_evidence(room_id, created_at);
