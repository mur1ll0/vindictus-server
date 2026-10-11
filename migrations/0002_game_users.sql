-- Initial dbo.User subset for the explicitly restricted ASCII lab adapter.
-- Not an import of all 15 legacy columns, characters, credentials or T-SQL effects.
CREATE TABLE game_users (
    id INTEGER PRIMARY KEY,
    name TEXT COLLATE "C" NOT NULL,
    name_key TEXT COLLATE "C" GENERATED ALWAYS AS (
        translate(rtrim(name, ' '),
                  'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz')
    ) STORED UNIQUE,
    next_character_sn INTEGER NOT NULL,
    service_id INTEGER NOT NULL DEFAULT -1,
    channeling_code SMALLINT,
    -- Operational metadata, NOT legacy CreateTime or SQL Server rowversion.
    persisted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT game_user_name_bound CHECK (octet_length(name) <= 24),
    CONSTRAINT game_user_name_ascii CHECK (name COLLATE "C" !~ '[^ -~]'),
    CONSTRAINT game_user_channel_bound CHECK (channeling_code BETWEEN 0 AND 255)
);
