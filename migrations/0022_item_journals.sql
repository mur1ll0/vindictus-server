-- Both source procedures consume identity values despite rolling back the counter row.
-- AddItemJournal deliberately calls getCharacterJournalID, not getItemJournalID.
CREATE SEQUENCE game_item_journal_id AS BIGINT START WITH 611635750 NO CYCLE;
CREATE SEQUENCE game_character_journal_id AS BIGINT START WITH 4419625 NO CYCLE;
CREATE TABLE game_journal_identity (
 kind SMALLINT PRIMARY KEY CHECK(kind IN(1,2)),
 source_last BIGINT,
 CHECK(source_last IS NULL OR (source_last >= CASE kind WHEN 1 THEN 611635750 ELSE 4419625 END AND source_last < 9223372036854775807))
);
CREATE TABLE game_item_journal01 (
 id BIGINT PRIMARY KEY,
 category_utf16 BYTEA NOT NULL CHECK(octet_length(category_utf16)<=100 AND octet_length(category_utf16)%2=0),
 time_ticks BIGINT NOT NULL DEFAULT character_event_log_ticks(floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT) CHECK(time_ticks BETWEEN 0 AND 3155378975999999999),
 legacy_last_update BYTEA CHECK(octet_length(legacy_last_update)=8)
);
CREATE UNIQUE INDEX game_item_journal01_rowversion ON game_item_journal01(legacy_last_update);
CREATE INDEX game_item_journal01_time ON game_item_journal01(time_ticks);
CREATE TABLE game_item_journal02 (LIKE game_item_journal01 INCLUDING DEFAULTS INCLUDING CONSTRAINTS INCLUDING INDEXES);
