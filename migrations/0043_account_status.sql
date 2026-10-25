-- Administration panel (B4.2): an account can be disabled with an audited reason. A disabled
-- account cannot log in to the panel (and, with the play sessions of B4.2, to the game).
ALTER TABLE accounts
    ADD COLUMN status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled'));
