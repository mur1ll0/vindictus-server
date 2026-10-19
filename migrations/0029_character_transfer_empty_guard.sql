-- All eight fields of the recovered empty CharacterInfo_Trans table.
-- Source PK(newCharacterID),no identity or FK; strings remain bounded UTF-16 bytes.
CREATE TABLE game_character_transfer_info(
 uid INTEGER NOT NULL,
 old_name_utf16 BYTEA NOT NULL CHECK(octet_length(old_name_utf16)<=100 AND octet_length(old_name_utf16)%2=0),
 old_cid BIGINT NOT NULL,old_sn INTEGER NOT NULL,
 new_name_utf16 BYTEA NOT NULL CHECK(octet_length(new_name_utf16)<=160 AND octet_length(new_name_utf16)%2=0),
 new_cid BIGINT PRIMARY KEY,new_sn INTEGER NOT NULL,
 change_name_utf16 BYTEA CHECK(octet_length(change_name_utf16)<=160 AND octet_length(change_name_utf16)%2=0)
);
CREATE TABLE game_character_transfer_guard(
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 source_sha256 BYTEA NOT NULL CHECK(octet_length(source_sha256)=32),
 schema_sha256 TEXT NOT NULL CHECK(schema_sha256 ~ '^[0-9a-f]{64}$'),
 procedure_sha256 TEXT NOT NULL CHECK(procedure_sha256='6f47c1f5f6926d7d5bd474fa91547c2e31343d3afc0f8853e3a0ffee0370baaa')
);
