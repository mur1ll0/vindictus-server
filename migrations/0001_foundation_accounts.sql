-- Independent preservation schema. Not a conversion of legacy SQL Server tables.
CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    username VARCHAR(32) NOT NULL,
    normalized_name VARCHAR(32) NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT account_username_format CHECK (username ~ '^[A-Za-z0-9_]{3,32}$'),
    CONSTRAINT account_name_normalized CHECK (normalized_name = lower(username)),
    CONSTRAINT account_password_format CHECK (password_hash LIKE '$argon2id$%')
);
