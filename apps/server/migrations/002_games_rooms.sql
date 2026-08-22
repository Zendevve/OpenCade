-- 002_games_rooms.sql — games, game_versions, servers, rooms, room_members, matches, reports, bans per ARCHITECTURE.md §9
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- games: canonical game catalog (spec: id TEXT PK, seeded from game-definitions)
CREATE TABLE IF NOT EXISTS games (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  emulator TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- game_versions: romset / version rows per game
CREATE TABLE IF NOT EXISTS game_versions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  version TEXT NOT NULL,
  is_default BOOLEAN NOT NULL DEFAULT false,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(game_id, version)
);

-- servers: region / relay servers
CREATE TABLE IF NOT EXISTS servers (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name TEXT NOT NULL,
  region TEXT NOT NULL,
  host TEXT NOT NULL,
  port INT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- rooms: match rooms, server-authoritative state machine
-- ARCHITECTURE.md §9 lists WAITING/READY/PLAYING/FINISHED/CANCELLED;
-- AGENTS.md Data Flow lists WAITING→CHALLENGING→CONNECTING→PLAYING→FINISHED|CANCELLED.
-- Superset CHECK keeps both histories valid so accept→CONNECTING→PLAYING QA loop never hits DB rejection.
CREATE TABLE IF NOT EXISTS rooms (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  game_id TEXT NOT NULL REFERENCES games(id),
  server_id UUID REFERENCES servers(id),
  host_user_id UUID NOT NULL REFERENCES users(id),
  state TEXT NOT NULL CHECK (state IN ('WAITING','READY','CHALLENGING','CONNECTING','PLAYING','FINISHED','CANCELLED')),
  max_players INT NOT NULL DEFAULT 2 CHECK (max_players BETWEEN 2 AND 4),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_rooms_game_state ON rooms(game_id, state);
CREATE INDEX IF NOT EXISTS idx_rooms_host_user_id ON rooms(host_user_id);

-- room_members: join table
CREATE TABLE IF NOT EXISTS room_members (
  room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  joined_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (room_id, user_id)
);

-- matches: finished play sessions
CREATE TABLE IF NOT EXISTS matches (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  room_id UUID NOT NULL REFERENCES rooms(id),
  game_id TEXT NOT NULL REFERENCES games(id),
  started_at TIMESTAMPTZ NOT NULL,
  ended_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- reports: user reports
CREATE TABLE IF NOT EXISTS reports (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  reporter_id UUID NOT NULL REFERENCES users(id),
  target_user_id UUID REFERENCES users(id),
  room_id UUID REFERENCES rooms(id),
  reason TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- bans: admin bans
CREATE TABLE IF NOT EXISTS bans (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  reason TEXT NOT NULL,
  banned_by UUID NOT NULL REFERENCES users(id),
  expires_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_bans_user_expires ON bans(user_id, expires_at);

-- updated_at trigger for rooms
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rooms_updated_at ON rooms;
CREATE TRIGGER trg_rooms_updated_at
  BEFORE UPDATE ON rooms
  FOR EACH ROW
  EXECUTE FUNCTION update_updated_at_column();

-- Seed: canonical games from packages/game-definitions (authoritative per §9, seeded at migration time)
INSERT INTO games (id, name, emulator) VALUES
  ('kof98', 'The King of Fighters ''98', 'fbneo'),
  ('sfiii3', 'Street Fighter III: 3rd Strike', 'fbneo'),
  ('garou', 'Garou: Mark of the Wolves', 'fbneo'),
  ('kof2002', 'The King of Fighters 2002', 'fbneo'),
  ('mvc2', 'Marvel vs. Capcom 2', 'flycast')
ON CONFLICT (id) DO NOTHING;

INSERT INTO game_versions (game_id, version, is_default) VALUES
  ('kof98', '0.2.97.44', true),
  ('sfiii3', '0.2.97.44', true),
  ('garou', '0.2.97.44', true),
  ('kof2002', '0.2.97.44', true),
  ('mvc2', '1.0', true)
ON CONFLICT (game_id, version) DO NOTHING;

INSERT INTO servers (name, region, host, port)
SELECT 'us-east-1', 'us-east', '127.0.0.1', 3478
WHERE NOT EXISTS (SELECT 1 FROM servers WHERE name='us-east-1');

INSERT INTO servers (name, region, host, port)
SELECT 'eu-west-1', 'eu-west', '127.0.0.1', 3479
WHERE NOT EXISTS (SELECT 1 FROM servers WHERE name='eu-west-1');
