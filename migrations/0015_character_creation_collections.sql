-- Recovered creation owners: source FK to Character, ON DELETE CASCADE ON UPDATE CASCADE.
CREATE TABLE game_item_collections (
    owner_id BIGINT PRIMARY KEY REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE,
    permission INTEGER NOT NULL DEFAULT 3
);
CREATE TABLE game_quest_collections (
    owner_id BIGINT PRIMARY KEY REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE,
    permission INTEGER NOT NULL DEFAULT 3
);
CREATE TABLE game_mailboxes (
    owner_id BIGINT PRIMARY KEY DEFAULT -1 REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE,
    permission INTEGER NOT NULL DEFAULT 3
);
CREATE TABLE game_story_entries (
    cid BIGINT PRIMARY KEY REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE,
    permission INTEGER NOT NULL DEFAULT 2,
    hint_category SMALLINT NOT NULL DEFAULT 0 CHECK (hint_category BETWEEN 0 AND 255)
);
-- Source varchar(255) bytes, including CP1252 values/trailing spaces.
-- MySettings has no primary key, identity, FK or uniqueness constraint.
CREATE TABLE game_my_settings (
    code_bytes BYTEA NOT NULL CHECK (octet_length(code_bytes) <= 255),
    value_bytes BYTEA NOT NULL CHECK (octet_length(value_bytes) <= 255)
);