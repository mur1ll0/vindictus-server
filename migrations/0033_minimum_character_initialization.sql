-- Explicit native laboratory scope;not a claim of complete legacy initialization.
CREATE TABLE game_minimum_character_initialization(
 cid BIGINT PRIMARY KEY REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE,
 uid INTEGER NOT NULL REFERENCES game_users(id) ON DELETE CASCADE ON UPDATE CASCADE,
 profile_sha256 BYTEA NOT NULL CHECK(octet_length(profile_sha256)=32),
 initialized BOOLEAN NOT NULL DEFAULT false
);
CREATE INDEX game_minimum_character_owner ON game_minimum_character_initialization(uid);
