-- Source Costume: RowID is the only unique key; duplicate appearance keys are preserved.
CREATE TABLE game_costumes (
 row_id BIGINT PRIMARY KEY,
 cid BIGINT NOT NULL REFERENCES game_characters(id) ON UPDATE CASCADE ON DELETE CASCADE,
 is_base BOOLEAN NOT NULL,
 costume_type SMALLINT NOT NULL CHECK(costume_type BETWEEN 0 AND 255),
 costume_key SMALLINT NOT NULL CHECK(costume_key BETWEEN 0 AND 255),
 costume_sn INTEGER, c1 INTEGER, c2 INTEGER, c3 INTEGER,
 gloss SMALLINT CHECK(gloss BETWEEN 0 AND 255), effect INTEGER
);
CREATE INDEX game_costumes_cid_idx ON game_costumes(cid);
-- Explicit bootstrap from sys.identity_columns.last_value, never MAX+1.
CREATE SEQUENCE game_costume_row_id AS bigint INCREMENT BY 1 MINVALUE 1 MAXVALUE 9223372036854775807 START WITH 1 CACHE 1 NO CYCLE;
ALTER SEQUENCE game_costume_row_id OWNED BY game_costumes.row_id;
CREATE TABLE game_costume_identity (
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 initialized BOOLEAN NOT NULL DEFAULT FALSE,
 source_last_value BIGINT CHECK(source_last_value BETWEEN 0 AND 9223372036854775806)
);
INSERT INTO game_costume_identity(singleton) VALUES(TRUE);
