-- Operational batch receipts; no legacy table or source snapshot is rewritten.
-- A transaction claims the UUID before reading the account or allocating queue identity.
-- No FK to queued rows: replay must survive processing/deletion and return historical IDs.
CREATE TABLE game_login_reward_runs (
    operation_id UUID PRIMARY KEY,
    uid INTEGER NOT NULL,
    name_key TEXT COLLATE "C" NOT NULL CHECK (octet_length(name_key) BETWEEN 0 AND 24),
    request BYTEA NOT NULL CHECK (octet_length(request) BETWEEN 1 AND 2048),
    revision_after BIGINT CHECK (revision_after >= 0),
    is_dormant BOOLEAN,
    is_return BOOLEAN,
    queued_row_ids INTEGER[],
    CHECK (
       (revision_after IS NULL AND is_dormant IS NULL AND is_return IS NULL AND queued_row_ids IS NULL)
       OR (revision_after IS NOT NULL AND is_dormant IS NOT NULL AND is_return IS NOT NULL
           AND queued_row_ids IS NOT NULL AND cardinality(queued_row_ids) <= 8192
           AND array_position(queued_row_ids, NULL) IS NULL)
    )
);
