-- Local laboratory provenance, not a recovered legacy table or relationship.
-- A marker is created only with a genuinely new user in the same transaction.
CREATE TABLE game_lab_account_origins (
    uid INTEGER PRIMARY KEY REFERENCES game_users(id) ON DELETE CASCADE,
    name_key TEXT COLLATE "C" NOT NULL CHECK (
        name_key ~ '^[ -~]{1,24}$' AND name_key = lower(name_key) AND name_key = rtrim(name_key,' ')
    ),
    profile_fingerprint BYTEA NOT NULL CHECK (octet_length(profile_fingerprint)=32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
