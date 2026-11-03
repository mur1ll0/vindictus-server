-- Game login through the launcher (trilha L, L-A/L4). The 1.69 client sends only its ID field as
-- the UserLogin passport and never the password, so the launcher checks login and password on the
-- panel listener and receives a one-time ticket; the client is started with -launcher_ticket and
-- its login dialog sends the ticket as the ID (scripts/client/patch_client_launcher_login.py); the
-- frontend redeems it once. Local laboratory state, not a recovered legacy table.

-- Only the SHA-256 of the ticket is stored; a ticket is used once and expires.
CREATE TABLE game_launcher_tickets (
    ticket_sha256 BYTEA PRIMARY KEY CHECK (octet_length(ticket_sha256) = 32),
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    redeemed_at TIMESTAMPTZ,
    CHECK (expires_at > created_at)
);
CREATE INDEX game_launcher_tickets_account ON game_launcher_tickets (account_id);
