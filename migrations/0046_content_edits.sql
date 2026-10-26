-- Content edits of the admin panel (B4.4): NPC shop rows (ShopInfo) and cash shop prices.
-- Kept apart from the characters' transactional state. An edit is pending until a version
-- publishes it; the host script (scripts/content/apply-content-edits.ps1) exports every
-- published edit, re-runs the content exporters, updates the client copy and restarts the
-- game, then records the version as applied. Edits are never deleted: withdrawing a pending
-- edit keeps it, and undoing a published one is a newer edit.
CREATE TABLE content_versions (
    version INTEGER PRIMARY KEY CHECK (version > 0),
    published_by UUID NOT NULL REFERENCES accounts(id),
    published_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reason TEXT NOT NULL CHECK (char_length(reason) BETWEEN 1 AND 500),
    applied_at TIMESTAMPTZ
);

CREATE TABLE content_edits (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    -- shop_row: add or change a ShopInfo row; shop_row_remove: remove it;
    -- cash_price: the price of an ItemClassInfo.Category; cash_default: the default price.
    kind TEXT NOT NULL CHECK (kind IN ('shop_row', 'shop_row_remove', 'cash_price', 'cash_default')),
    -- "<ShopID>#<Order>", the category name, or "default".
    target TEXT NOT NULL CHECK (char_length(target) BETWEEN 1 AND 128),
    value JSONB,
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'published', 'withdrawn')),
    version INTEGER REFERENCES content_versions(version),
    created_by UUID NOT NULL REFERENCES accounts(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reason TEXT NOT NULL CHECK (char_length(reason) BETWEEN 1 AND 500),
    CHECK ((state = 'published') = (version IS NOT NULL))
);
CREATE INDEX content_edits_state ON content_edits (state, id);
