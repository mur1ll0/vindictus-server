-- Quest, GoalAchievement and QuestClearTime of the quest ownership service (owner rows of
-- QuestCollection). DateTime columns keep SQL datetime 1/300 s units.
CREATE TABLE game_quests (
 row_id BIGINT PRIMARY KEY,
 owner_id BIGINT NOT NULL REFERENCES game_quest_collections(owner_id) ON DELETE CASCADE ON UPDATE CASCADE,
 quest_utf16 BYTEA NOT NULL CHECK(octet_length(quest_utf16)<=100 AND octet_length(quest_utf16)%2=0),
 revealed INTEGER NOT NULL,
 status INTEGER NOT NULL,
 clear_count INTEGER NOT NULL,
 play_count INTEGER NOT NULL,
 last_play_units300 BIGINT NOT NULL CHECK(last_play_units300 BETWEEN -1391644800000 AND 76683386879999),
 today_play_count INTEGER NOT NULL,
 named_kill_units300 BIGINT CHECK(named_kill_units300 BETWEEN -1391644800000 AND 76683386879999)
);
-- QuestOwner.Quests.Add throws on a duplicate QuestID of one owner.
CREATE UNIQUE INDEX game_quests_owner_quest ON game_quests(owner_id,quest_utf16);
CREATE TABLE game_quest_goal_achievements (
 row_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 owner_id BIGINT NOT NULL REFERENCES game_quest_collections(owner_id) ON DELETE CASCADE ON UPDATE CASCADE,
 goal_id INTEGER NOT NULL,
 status INTEGER NOT NULL,
 clear_count SMALLINT NOT NULL CHECK(clear_count BETWEEN 0 AND 255),
 regened_ap SMALLINT NOT NULL CHECK(regened_ap BETWEEN 0 AND 255),
 last_clear_units300 BIGINT NOT NULL CHECK(last_clear_units300 BETWEEN -1391644800000 AND 76683386879999)
);
CREATE UNIQUE INDEX game_quest_goal_achievements_owner_goal ON game_quest_goal_achievements(owner_id,goal_id);
CREATE TABLE game_quest_clear_times (
 row_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 owner_id BIGINT NOT NULL REFERENCES game_quest_collections(owner_id) ON DELETE CASCADE ON UPDATE CASCADE,
 quest_utf16 BYTEA NOT NULL CHECK(octet_length(quest_utf16)<=100 AND octet_length(quest_utf16)%2=0),
 difficulty INTEGER NOT NULL,
 player_count INTEGER NOT NULL,
 clear_time INTEGER NOT NULL,
 play_rank INTEGER NOT NULL,
 clear_info INTEGER NOT NULL,
 is_old SMALLINT NOT NULL CHECK(is_old BETWEEN 0 AND 255)
);
CREATE INDEX game_quest_clear_times_owner ON game_quest_clear_times(owner_id,row_id);
