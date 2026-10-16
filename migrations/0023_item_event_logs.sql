-- Event rows have no source PK/FK or uniqueness on journal/item IDs.
-- Original rowversions only; locally appended events keep legacy_last_update NULL.
CREATE TABLE game_item_log01 (
 journal_id BIGINT NOT NULL,character_id BIGINT NOT NULL,event SMALLINT NOT NULL,detail SMALLINT NOT NULL,
 before_item_id BIGINT,item_id BIGINT NOT NULL,
 item_class_utf16 BYTEA NOT NULL CHECK(octet_length(item_class_utf16)<=2048 AND octet_length(item_class_utf16)%2=0),
 count INTEGER NOT NULL,after_inventory INTEGER,after_shared INTEGER,
 color1 INTEGER,color2 INTEGER,color3 INTEGER,reduced_durability INTEGER,max_durability_bonus INTEGER,
 memo_utf16 BYTEA CHECK(octet_length(memo_utf16)<=2048 AND octet_length(memo_utf16)%2=0),
 time_units300 BIGINT NOT NULL CHECK(time_units300 BETWEEN -1391644800000 AND 76683386879999),
 legacy_last_update BYTEA CHECK(octet_length(legacy_last_update)=8)
);
CREATE INDEX game_item_log01_journal ON game_item_log01(journal_id);
CREATE INDEX game_item_log01_time ON game_item_log01(time_units300);
CREATE INDEX game_item_log01_rowversion ON game_item_log01(legacy_last_update);
CREATE TABLE game_item_log02 (LIKE game_item_log01 INCLUDING DEFAULTS INCLUDING CONSTRAINTS INCLUDING INDEXES);
CREATE TABLE game_item_ledger01 (
 journal_id BIGINT NOT NULL,item_id BIGINT NOT NULL,
 item_class_utf16 BYTEA NOT NULL CHECK(octet_length(item_class_utf16)<=2048 AND octet_length(item_class_utf16)%2=0),
 character_id BIGINT NOT NULL,event SMALLINT NOT NULL,count INTEGER NOT NULL DEFAULT 1,
 time_units300 BIGINT NOT NULL DEFAULT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT CHECK(time_units300 BETWEEN -1391644800000 AND 76683386879999),
 legacy_last_update BYTEA CHECK(octet_length(legacy_last_update)=8),
 color1 INTEGER,color2 INTEGER,color3 INTEGER,reduced_durability INTEGER,max_durability_bonus INTEGER,
 action SMALLINT NOT NULL DEFAULT 0,bundle_item_count INTEGER
);
CREATE UNIQUE INDEX game_item_ledger01_rowversion ON game_item_ledger01(legacy_last_update);
CREATE TABLE game_item_ledger02 (LIKE game_item_ledger01 INCLUDING DEFAULTS INCLUDING CONSTRAINTS INCLUDING INDEXES);
