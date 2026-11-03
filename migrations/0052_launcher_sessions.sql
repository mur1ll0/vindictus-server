-- Launcher sessions (trilha L, operator decision 2026-11-02): the player logs in once in the
-- launcher and may start the game much later, so a game ticket no longer expires after two
-- minutes. Login opens a launcher session (bearer token, only its SHA-256 stored) that lasts until
-- the launcher logs out on close, the account's password changes or the account is disabled, or
-- at most a fixed lifetime if the launcher dies. Each Play mints a one-time ticket bound to the
-- session; it lives as long as the session and is removed with it.
CREATE TABLE launcher_sessions (
    token_sha256 BYTEA PRIMARY KEY CHECK (octet_length(token_sha256) = 32),
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    password_changed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    CHECK (expires_at > created_at)
);
CREATE INDEX launcher_sessions_account ON launcher_sessions (account_id);

-- Tickets of 0051 had no session; they were short-lived laboratory tickets.
DELETE FROM game_launcher_tickets;
ALTER TABLE game_launcher_tickets
    ADD COLUMN session_sha256 BYTEA NOT NULL
        REFERENCES launcher_sessions(token_sha256) ON DELETE CASCADE;
CREATE INDEX game_launcher_tickets_session ON game_launcher_tickets (session_sha256);
