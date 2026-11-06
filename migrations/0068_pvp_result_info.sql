-- PvP results (B5 PV4): heroes PvpResultInfo (CID, PvpMode, Win, Lose,
-- MaxSuccessiveWinCount), merged by PvpResultInfoUpdate and read by PvpResultInfoGet.
CREATE TABLE game_pvp_result_info(
 cid BIGINT NOT NULL,
 pvp_mode TEXT NOT NULL CHECK(char_length(pvp_mode)<=50),
 win INTEGER NOT NULL,
 lose INTEGER NOT NULL,
 max_successive_win_count INTEGER,
 PRIMARY KEY(cid, pvp_mode)
);
