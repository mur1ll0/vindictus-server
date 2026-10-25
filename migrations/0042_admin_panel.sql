-- Administration panel (B4): local operational state, not a recovered legacy table.
-- The game account doubles as the panel login; a NULL role has no panel access.
ALTER TABLE accounts
    ADD COLUMN panel_role TEXT CHECK (panel_role IN ('admin', 'gm', 'leitura')),
    ADD COLUMN failed_logins INTEGER NOT NULL DEFAULT 0 CHECK (failed_logins BETWEEN 0 AND 1000),
    ADD COLUMN locked_until TIMESTAMPTZ,
    ADD COLUMN password_changed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;

-- Only the SHA-256 of the cookie token is stored. A session ends when its account's password
-- changes (password_changed_at differs) or its role is removed.
CREATE TABLE admin_sessions (
    token_sha256 BYTEA PRIMARY KEY CHECK (octet_length(token_sha256) = 32),
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    password_changed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    CHECK (expires_at > created_at)
);
CREATE INDEX admin_sessions_account ON admin_sessions (account_id);

-- Every panel change and every login attempt. Append-only: the trigger refuses UPDATE,
-- DELETE and TRUNCATE. account_id/actor_name are NULL for operator CLI changes and for
-- logins with an unknown name.
CREATE TABLE admin_audit_events (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    account_id UUID REFERENCES accounts(id),
    actor_name VARCHAR(32),
    role TEXT CHECK (role IN ('admin', 'gm', 'leitura')),
    operation_id UUID UNIQUE,
    action TEXT NOT NULL CHECK (action ~ '^[a-z_]{1,32}(\.[a-z_]{1,32}){0,2}$'),
    target_kind TEXT CHECK (target_kind ~ '^[a-z_]{1,32}$'),
    target_id TEXT CHECK (octet_length(target_id) <= 128),
    reason TEXT CHECK (char_length(reason) BETWEEN 1 AND 500),
    before JSONB,
    after JSONB,
    outcome TEXT NOT NULL CHECK (outcome IN ('ok', 'refused')),
    peer INET
);
CREATE INDEX admin_audit_events_target ON admin_audit_events (target_kind, target_id, id);
CREATE INDEX admin_audit_events_account ON admin_audit_events (account_id, id);

CREATE FUNCTION admin_audit_events_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'admin_audit_events is append-only';
END
$$;
CREATE TRIGGER admin_audit_events_no_update_delete
    BEFORE UPDATE OR DELETE ON admin_audit_events
    FOR EACH ROW EXECUTE FUNCTION admin_audit_events_append_only();
CREATE TRIGGER admin_audit_events_no_truncate
    BEFORE TRUNCATE ON admin_audit_events
    FOR EACH STATEMENT EXECUTE FUNCTION admin_audit_events_append_only();
