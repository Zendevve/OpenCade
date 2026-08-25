-- Native-process lifecycle evidence and the MVP's two-player invariant.
UPDATE rooms SET max_players = 2 WHERE max_players <> 2;
ALTER TABLE rooms DROP CONSTRAINT IF EXISTS rooms_max_players_check;
ALTER TABLE rooms
  ADD CONSTRAINT rooms_max_players_check CHECK (max_players = 2);

CREATE UNIQUE INDEX IF NOT EXISTS idx_matches_room_unique ON matches (room_id);

CREATE TABLE IF NOT EXISTS match_runtime_participants (
  room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  launched_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  ended_at TIMESTAMPTZ,
  exit_code INT,
  PRIMARY KEY (room_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_match_runtime_room_ended
  ON match_runtime_participants (room_id, ended_at);

CREATE TABLE IF NOT EXISTS match_launch_grants (
  token_hash TEXT PRIMARY KEY,
  room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
  game_id TEXT NOT NULL REFERENCES games(id),
  local_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  peer_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('host', 'guest')),
  local_endpoint TEXT NOT NULL,
  peer_endpoint TEXT NOT NULL,
  input_delay_frames SMALLINT NOT NULL CHECK (input_delay_frames BETWEEN 0 AND 15),
  expires_at TIMESTAMPTZ NOT NULL,
  consumed_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_match_launch_grants_expiry
  ON match_launch_grants (expires_at);

CREATE INDEX IF NOT EXISTS idx_match_launch_grants_room_user
  ON match_launch_grants (room_id, local_user_id, expires_at);
