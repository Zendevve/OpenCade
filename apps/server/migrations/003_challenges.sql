-- 003_challenges.sql — durable, user-addressed room challenges.
CREATE TABLE IF NOT EXISTS challenges (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  room_id UUID NOT NULL UNIQUE REFERENCES rooms(id) ON DELETE CASCADE,
  challenger_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  challenged_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  state TEXT NOT NULL CHECK (state IN ('PENDING','ACCEPTED','DECLINED','CANCELLED')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (challenger_id <> challenged_id)
);

CREATE INDEX IF NOT EXISTS idx_challenges_challenged_state
  ON challenges(challenged_id, state, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_challenges_challenger_state
  ON challenges(challenger_id, state, created_at DESC);

DROP TRIGGER IF EXISTS trg_challenges_updated_at ON challenges;
CREATE TRIGGER trg_challenges_updated_at
  BEFORE UPDATE ON challenges
  FOR EACH ROW
  EXECUTE FUNCTION update_updated_at_column();
