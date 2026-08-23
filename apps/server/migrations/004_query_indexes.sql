-- 004_query_indexes.sql — indexes for authenticated lookup and match/lobby hot paths.
CREATE INDEX IF NOT EXISTS idx_users_lower_username ON users (lower(username));
CREATE INDEX IF NOT EXISTS idx_users_lower_email ON users (lower(email)) WHERE email IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_room_members_user_id ON room_members (user_id, room_id);
CREATE INDEX IF NOT EXISTS idx_rooms_host_game_state_created
  ON rooms (host_user_id, game_id, state, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_matches_room_open
  ON matches (room_id) WHERE ended_at IS NULL;
