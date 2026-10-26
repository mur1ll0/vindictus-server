-- Game login with login and password (B4.2). The client sends only the text of its ID field
-- as the passport (client.dll 0x38522400 with NGM_disable; the password field never reaches
-- the server) and `-passport` goes through the Nexon SSO. So the account logs in on the panel
-- listener and receives a one-time play code to type in the ID field; the frontend redeems it.
-- Local laboratory state, not a recovered legacy table.

-- The game account name (NexonID) an account enters; NULL = its own login. Lets an account
-- keep a game user created before accounts existed (lab169_a). One account per game name.
ALTER TABLE accounts
    ADD COLUMN game_name TEXT CHECK (game_name ~ '^[A-Za-z0-9_]{1,24}$');
CREATE UNIQUE INDEX accounts_game_identity ON accounts ((lower(COALESCE(game_name, username))));

-- Only the SHA-256 of the code is stored; a code is used once and expires.
CREATE TABLE game_play_codes (
    code_sha256 BYTEA PRIMARY KEY CHECK (octet_length(code_sha256) = 32),
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    redeemed_at TIMESTAMPTZ,
    CHECK (expires_at > created_at)
);
CREATE INDEX game_play_codes_account ON game_play_codes (account_id);
