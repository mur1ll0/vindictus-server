-- dbo.StatusEffect reference; original SQL Server remains read-only.
CREATE TABLE game_status_effects(
 row_id BIGINT PRIMARY KEY,
 cid BIGINT NOT NULL REFERENCES game_characters(id) ON UPDATE CASCADE ON DELETE CASCADE,
 type_utf16 BYTEA NOT NULL CHECK(octet_length(type_utf16)<=100 AND octet_length(type_utf16)%2=0),
 level INTEGER NOT NULL,remain_time INTEGER,
 expire_ticks BIGINT CHECK(expire_ticks BETWEEN 0 AND 3155378975999999999),
 remain_micro SMALLINT CHECK(remain_micro BETWEEN 0 AND 255)
);
-- Type is not a unique key in the source. Keep every RowID and raw UTF16 value.
CREATE INDEX game_status_effects_cid_idx ON game_status_effects(cid);
CREATE SEQUENCE game_status_effect_row_id AS bigint MINVALUE 1 MAXVALUE 9223372036854775807 START 1 CACHE 1 NO CYCLE;
ALTER SEQUENCE game_status_effect_row_id OWNED BY game_status_effects.row_id;
CREATE TABLE game_status_effect_preservation(
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),initialized BOOLEAN NOT NULL DEFAULT FALSE,
 source_last_value BIGINT CHECK(source_last_value BETWEEN 0 AND 9223372036854775806),
 source_sha256 BYTEA CHECK(octet_length(source_sha256)=32),
 CHECK(NOT initialized OR source_sha256 IS NOT NULL)
);
INSERT INTO game_status_effect_preservation(singleton) VALUES(TRUE);
