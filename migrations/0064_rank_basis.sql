-- Character ranking basis (B5 ranking RK1): heroesRank dbo.RankBasis (rowid identity, CID,
-- EventID, Score, LastUpdate, EntityID) and dbo.RankBasis_History (seqid identity, rowid,
-- Score, LastUpdate, EntityID), kept by RankBasisUpdate (study_heroesRank.schema.json). Times
-- are Unix seconds (the procedure's GETUTCDATE()). No foreign keys, as the source schema.
CREATE TABLE game_rank_basis(
 row_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 cid BIGINT NOT NULL,
 event_id INTEGER NOT NULL,
 score BIGINT NOT NULL,
 last_update_seconds BIGINT NOT NULL,
 entity_id BIGINT
);
CREATE INDEX game_rank_basis_owner ON game_rank_basis(cid, event_id, row_id);
CREATE INDEX game_rank_basis_event ON game_rank_basis(event_id);
CREATE TABLE game_rank_basis_history(
 seq_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 row_id BIGINT NOT NULL,
 score BIGINT NOT NULL,
 last_update_seconds BIGINT NOT NULL,
 entity_id BIGINT
);
CREATE INDEX game_rank_basis_history_row ON game_rank_basis_history(row_id);
