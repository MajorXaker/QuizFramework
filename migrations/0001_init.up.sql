-- Quiz Helper schema.
-- Sessions are short-lived (max 6h) and tracked in PostgreSQL so that
-- multiple backend replicas can share state if desired. Round/answer
-- evaluation latency is measured client-side and recorded here.

CREATE TABLE IF NOT EXISTS sessions (
    id              UUID PRIMARY KEY,
    invite_code     TEXT UNIQUE NOT NULL,
    host_id         UUID NOT NULL,
    joins_enabled   BOOLEAN NOT NULL DEFAULT TRUE,
    answers_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,
    closed_at       TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_sessions_invite_code ON sessions (invite_code);
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions (expires_at);

CREATE TABLE IF NOT EXISTS participants (
    id          UUID PRIMARY KEY,
    session_id  UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    is_host     BOOLEAN NOT NULL DEFAULT FALSE,
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    kicked_at   TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_participants_session ON participants (session_id);

CREATE TABLE IF NOT EXISTS rounds (
    id           UUID PRIMARY KEY,
    session_id   UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    started_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    stopped_at   TIMESTAMPTZ,
    winner_id    UUID REFERENCES participants(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_rounds_session ON rounds (session_id);

CREATE TABLE IF NOT EXISTS answers (
    id              UUID PRIMARY KEY,
    round_id        UUID NOT NULL REFERENCES rounds(id) ON DELETE CASCADE,
    participant_id  UUID NOT NULL REFERENCES participants(id) ON DELETE CASCADE,
    elapsed_ms      INTEGER NOT NULL,
    server_received TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_answers_round ON answers (round_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_answers_round_participant
    ON answers (round_id, participant_id);
