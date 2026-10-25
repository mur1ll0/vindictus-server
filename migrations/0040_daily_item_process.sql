-- DailyItemProcess of the character ItemLoader: the last ProcessedTime (.NET ticks, UTC)
-- of each DailyItemProcessInfo procedure (GetDailyItemProcess/UpdateDailyItemProcess).
CREATE TABLE game_daily_item_process (
 cid BIGINT NOT NULL REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE,
 process_id INTEGER NOT NULL,
 processed_ticks BIGINT NOT NULL CHECK(processed_ticks BETWEEN 0 AND 3155378975999999999),
 PRIMARY KEY(cid, process_id)
);
