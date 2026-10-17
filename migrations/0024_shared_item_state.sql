-- Source PKs only; no FK/owner-slot uniqueness exists on these two tables.
CREATE TABLE game_shared_items(id BIGINT PRIMARY KEY,owner_uid INTEGER,collection SMALLINT CHECK(collection BETWEEN 0 AND 255),slot INTEGER);
CREATE SEQUENCE game_shared_storage_id AS BIGINT START WITH 1 NO CYCLE;
CREATE TABLE game_shared_storage_status(id BIGINT PRIMARY KEY DEFAULT nextval('game_shared_storage_id'),owner_uid INTEGER NOT NULL,storage_type SMALLINT NOT NULL CHECK(storage_type BETWEEN 0 AND 255),storage SMALLINT NOT NULL CHECK(storage BETWEEN 0 AND 255),expire_units300 BIGINT CHECK(expire_units300 BETWEEN -1391644800000 AND 76683386879999));
CREATE TABLE game_shared_item_preservation(singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK(singleton),source_sha256 BYTEA NOT NULL CHECK(octet_length(source_sha256)=32),source_last BIGINT CHECK(source_last>=0 AND source_last<9223372036854775807));
