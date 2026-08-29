-- Evidence-first alpha opportunities: controller preflight, receipts, and the no-ROM fixture.

ALTER TABLE match_preflights
    ADD COLUMN IF NOT EXISTS controller_connected BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS player_slot SMALLINT;

ALTER TABLE match_preflights DROP CONSTRAINT IF EXISTS match_preflights_player_slot_check;
ALTER TABLE match_preflights ADD CONSTRAINT match_preflights_player_slot_check
    CHECK (player_slot IS NULL OR player_slot IN (1, 2));

CREATE TABLE IF NOT EXISTS match_receipts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    next_room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (source_room_id, created_by)
);

-- Bound the lifecycle reconciler's ordered lock scan to active deadlines.
CREATE INDEX IF NOT EXISTS idx_rooms_active_deadline
    ON rooms(state_deadline_at)
    WHERE state IN ('WAITING', 'CHALLENGING', 'CONNECTING', 'PLAYING')
      AND state_deadline_at IS NOT NULL;

-- Route policy reads are filtered by these immutable evidence dimensions.
-- Keeping room_id in the index supports the per-room aggregation without indexing
-- the full JSON payload.
CREATE INDEX IF NOT EXISTS idx_alpha_evidence_route_game_room
    ON alpha_evidence (
        (payload->>'native_route'),
        (payload->'room'->>'game_id'),
        room_id
    );

INSERT INTO games (id, name, emulator) VALUES
    ('opencade_test', 'OpenCade Proof-of-Play Test', 'retroarch_test')
ON CONFLICT (id) DO NOTHING;

INSERT INTO game_versions (game_id, version, is_default) VALUES
    ('opencade_test', '1.0.0', true)
ON CONFLICT (game_id, version) DO NOTHING;
