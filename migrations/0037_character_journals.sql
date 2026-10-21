-- CharacterJournal01/02 of AddCharacterJournal (CharacterEntity.OpenLog): the journal ID
-- comes from getCharacterJournalID (game_character_journal_id); Issued keeps SQL datetime
-- 1/300 s units (GETDATE civil offset measured zero). Rowversions are not fabricated.
CREATE TABLE game_character_journal01 (
 id BIGINT PRIMARY KEY,
 character_id BIGINT NOT NULL,
 issued_units300 BIGINT NOT NULL DEFAULT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT CHECK(issued_units300 BETWEEN -1391644800000 AND 76683386879999),
 legacy_last_update BYTEA CHECK(octet_length(legacy_last_update)=8)
);
CREATE UNIQUE INDEX game_character_journal01_rowversion ON game_character_journal01(legacy_last_update);
CREATE INDEX game_character_journal01_issued ON game_character_journal01(issued_units300);
CREATE TABLE game_character_journal02 (LIKE game_character_journal01 INCLUDING DEFAULTS INCLUDING CONSTRAINTS INCLUDING INDEXES);
