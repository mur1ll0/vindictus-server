-- Published rankings (B5 ranking RK2, decision D-S3 of 04/11/2026): the RankResultCharacter,
-- RankResultCharacterWeekly and RankResultCharacterMonthly tables of heroesRank in one table by
-- PeriodType (1, 2, 3) with n4Order, n4Rank, n4Rank_Prev and IsRead; the cumulative Count
-- scores at each 07:00 UTC boundary (the start of a Count window); the last published boundary;
-- RankFavoriteBasis (CID, EventID, LastUpdate) and the published RankResultHeavyFavorites.
-- Times are Unix seconds. No foreign keys, as the source schema.
CREATE TABLE game_rank_result(
 period_type SMALLINT NOT NULL CHECK(period_type BETWEEN 1 AND 3),
 event_id INTEGER NOT NULL,
 cid BIGINT NOT NULL,
 score BIGINT NOT NULL,
 n4_order INTEGER NOT NULL,
 n4_rank INTEGER NOT NULL,
 n4_rank_prev INTEGER NOT NULL,
 is_read BOOLEAN NOT NULL DEFAULT false,
 PRIMARY KEY(period_type, event_id, cid)
);
CREATE INDEX game_rank_result_cid ON game_rank_result(cid, period_type);
CREATE TABLE game_rank_count_snapshot(
 boundary_seconds BIGINT NOT NULL,
 cid BIGINT NOT NULL,
 event_id INTEGER NOT NULL,
 score BIGINT NOT NULL,
 PRIMARY KEY(boundary_seconds, event_id, cid)
);
CREATE TABLE game_rank_publication(
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 last_boundary_seconds BIGINT NOT NULL
);
CREATE TABLE game_rank_favorite(
 cid BIGINT NOT NULL,
 event_id INTEGER NOT NULL,
 last_update_seconds BIGINT NOT NULL,
 PRIMARY KEY(cid, event_id)
);
CREATE TABLE game_rank_heavy_favorite(
 event_id INTEGER PRIMARY KEY,
 favorites BIGINT NOT NULL,
 n4_rank INTEGER NOT NULL,
 n4_rank_prev INTEGER NOT NULL
);
