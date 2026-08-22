-- 002_games_rooms.sql — games, servers, rooms, chat_messages
-- M1 core matchmaking + chat schema. Idempotent via IF NOT EXISTS / ON CONFLICT.

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- games: canonical game catalog (seeded from packages/game-definitions)
CREATE TABLE IF NOT EXISTS games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    emulator TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- servers: region/host registry for relay / region selection
CREATE TABLE IF NOT EXISTS servers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    region TEXT NOT NULL,
    host TEXT NOT NULL,
    port INT NOT NULL,
    weight INT DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- rooms: match rooms, server-authoritative state machine
-- states: waiting -> challenging -> connecting -> playing -> finished / cancelled
CREATE TABLE IF NOT EXISTS rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID REFERENCES games(id),
    host_id UUID REFERENCES users(id) ON DELETE CASCADE,
    guest_id UUID REFERENCES users(id) ON DELETE SET NULL,
    state TEXT NOT NULL CHECK (state IN ('waiting','challenging','connecting','playing','finished','cancelled')),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- chat_messages: per-room chat history
CREATE TABLE IF NOT EXISTS chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id UUID REFERENCES rooms(id) ON DELETE CASCADE,
    author_id UUID REFERENCES users(id),
    body TEXT NOT NULL CHECK (char_length(body) BETWEEN 1 AND 2000),
    created_at TIMESTAMPTZ DEFAULT now()
);

-- Indexes for matchmaking and chat lookups
CREATE INDEX IF NOT EXISTS idx_rooms_game_id ON rooms(game_id);
CREATE INDEX IF NOT EXISTS idx_rooms_host_id ON rooms(host_id);
CREATE INDEX IF NOT EXISTS idx_chat_room_id ON chat_messages(room_id);

-- Additional helper indexes (idempotent)
CREATE INDEX IF NOT EXISTS idx_games_slug ON games(slug);
CREATE INDEX IF NOT EXISTS idx_servers_region ON servers(region);
CREATE INDEX IF NOT EXISTS idx_rooms_guest_id ON rooms(guest_id);
CREATE INDEX IF NOT EXISTS idx_rooms_state ON rooms(state);
CREATE INDEX IF NOT EXISTS idx_chat_messages_created_at ON chat_messages(created_at);

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

-- Seed: canonical game required by tests
INSERT INTO games (slug, name, emulator) VALUES ('sfiii3', 'Street Fighter III: 3rd Strike', 'fbneo') ON CONFLICT DO NOTHING;
