-- CharacterLedger01/02: six source columns, no PK/identity/FK/uniqueness.
-- GETDATE civil offset was measured zero in this reference. New source rowversions are not fabricated.
CREATE TABLE game_character_ledger01 (
    journal_id BIGINT NOT NULL,
    event_utf16 BYTEA NOT NULL CHECK (octet_length(event_utf16)<=100 AND octet_length(event_utf16)%2=0),
    target_utf16 BYTEA DEFAULT '\x'::bytea CHECK (octet_length(target_utf16)<=512 AND octet_length(target_utf16)%2=0),
    value INTEGER NOT NULL,
    time_units300 BIGINT NOT NULL DEFAULT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT CHECK(time_units300 BETWEEN -1391644800000 AND 76683386879999),
    legacy_last_update BYTEA CHECK(octet_length(legacy_last_update)=8)
);
CREATE TABLE game_character_ledger02 (LIKE game_character_ledger01 INCLUDING DEFAULTS INCLUDING CONSTRAINTS);
