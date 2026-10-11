-- Separate local optimistic revision; copied SQL Server rowversion is never regenerated.
ALTER TABLE game_users ADD COLUMN local_revision BIGINT NOT NULL DEFAULT 0 CHECK (local_revision >= 0);
CREATE FUNCTION bump_game_user_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.local_revision <> OLD.local_revision THEN
        RAISE EXCEPTION 'local revision is database-managed' USING ERRCODE='23514';
    END IF;
    IF OLD.local_revision = 9223372036854775807 THEN
        RAISE EXCEPTION 'local revision exhausted' USING ERRCODE='23514';
    END IF;
    NEW.local_revision := OLD.local_revision + 1;
    RETURN NEW;
END;
$$;
CREATE TRIGGER game_user_revision BEFORE UPDATE ON game_users
    FOR EACH ROW EXECUTE FUNCTION bump_game_user_revision();

-- ASCII subset of legacy UserBan.ID collation; no FK: source bans may precede a user.
CREATE TABLE game_user_bans (
    name TEXT COLLATE "C" NOT NULL CHECK (octet_length(name) <= 50 AND name !~ '[^ -~]'),
    name_key TEXT COLLATE "C" GENERATED ALWAYS AS (
        translate(rtrim(name, ' '), 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz')
    ) STORED PRIMARY KEY,
    status INTEGER NOT NULL,
    expire_time_units300 BIGINT CHECK (expire_time_units300 BETWEEN -1391644800000 AND 76683386879999),
    reason_utf16 BYTEA CHECK (octet_length(reason_utf16) <= 512 AND octet_length(reason_utf16) % 2 = 0),
    source_rowversion BYTEA CHECK (octet_length(source_rowversion) = 8)
);
