-- Opt-in, anonymous product activation telemetry. Raw events expire after 90 days.
CREATE TABLE product_events (
    event_id UUID PRIMARY KEY,
    anonymous_session_id UUID NOT NULL,
    event_name TEXT NOT NULL CHECK (
        event_name IN ('game_selected', 'readiness_completed', 'readiness_blocked', 'lobby_entered')
    ),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    blocked_checks JSONB NOT NULL DEFAULT '[]'::jsonb
        CHECK (jsonb_typeof(blocked_checks) = 'array'),
    received_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_product_events_received_at
    ON product_events (received_at DESC);
CREATE INDEX idx_product_events_session_received_at
    ON product_events (anonymous_session_id, game_id, received_at DESC);
