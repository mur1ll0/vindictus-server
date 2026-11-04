-- Whisper filter (B5 social SO2): dbo.WhisperBlock of the character database (PlayerService
-- WhisperBlock.cs: RowID identity, FromCID, ToCID, BlockLevel). WhisperFilter adds a row only
-- when the pair is absent and removes every row of the pair, so the pair is unique here; no
-- foreign keys, as the source schema.
CREATE TABLE game_whisper_block(
 row_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 from_cid BIGINT NOT NULL,
 to_cid BIGINT NOT NULL,
 block_level INTEGER NOT NULL,
 UNIQUE(from_cid, to_cid)
);
