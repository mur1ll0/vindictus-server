-- Restricted initializer storage: original source columns, primary keys and FK actions.
-- No fabricated mission FK, defaults or identity. Historical import remains separate.
CREATE TABLE game_titles (
 cid BIGINT NOT NULL REFERENCES game_characters(id) ON UPDATE CASCADE ON DELETE CASCADE,
 title_id INTEGER NOT NULL,
 acquired BOOLEAN NOT NULL,
 acquired_time_units300 BIGINT CHECK(acquired_time_units300 BETWEEN -1391644800000 AND 76683386879999),
 acquired_quest_utf16 BYTEA CHECK(octet_length(acquired_quest_utf16)<=100 AND octet_length(acquired_quest_utf16)%2=0),
 expire_ticks BIGINT CHECK(expire_ticks BETWEEN 0 AND 3155378975999999999),
 PRIMARY KEY(cid,title_id)
);
CREATE TABLE game_title_goal_progress (
 cid BIGINT NOT NULL REFERENCES game_characters(id) ON UPDATE CASCADE ON DELETE CASCADE,
 title_goal_id INTEGER NOT NULL,
 progress INTEGER NOT NULL,
 PRIMARY KEY(cid,title_goal_id)
);
CREATE TABLE game_title_presets (
 title_id INTEGER PRIMARY KEY,
 class_restriction INTEGER NOT NULL
);
CREATE TABLE game_micro_play_status (
 cid BIGINT PRIMARY KEY REFERENCES game_characters(id) ON UPDATE CASCADE ON DELETE CASCADE,
 micro_play_count INTEGER NOT NULL,
 micro_play_time BIGINT NOT NULL,
 success_micro_play_count INTEGER NOT NULL,
 success_micro_play_time BIGINT NOT NULL,
 daily_micro_play_count INTEGER NOT NULL,
 daily_free_play_count INTEGER NOT NULL,
 last_micro_play_started_units300 BIGINT NOT NULL CHECK(last_micro_play_started_units300 BETWEEN -1391644800000 AND 76683386879999),
 daily_report_count INTEGER NOT NULL,
 last_reported_time_units300 BIGINT NOT NULL CHECK(last_reported_time_units300 BETWEEN -1391644800000 AND 76683386879999),
 current_quest_utf16 BYTEA CHECK(octet_length(current_quest_utf16)<=100 AND octet_length(current_quest_utf16)%2=0),
 report_received_count INTEGER
);
CREATE TABLE game_character_mission_info (
 cid BIGINT PRIMARY KEY,
 completed_mission_count INTEGER NOT NULL,
 last_clear_ticks BIGINT CHECK(last_clear_ticks BETWEEN 0 AND 3155378975999999999)
);
CREATE TABLE game_character_state_preservation (
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 initialized BOOLEAN NOT NULL DEFAULT FALSE,
 source_sha256 BYTEA CHECK(octet_length(source_sha256)=32),
 source_rows BIGINT[] CHECK(array_length(source_rows,1)=6 AND 0<=ALL(source_rows))
);
INSERT INTO game_character_state_preservation(singleton) VALUES(TRUE);
CREATE TABLE game_mission_assignments (
 cid BIGINT NOT NULL,
 id BIGINT NOT NULL,
 mission_utf16 BYTEA NOT NULL CHECK(octet_length(mission_utf16)<=256 AND octet_length(mission_utf16)%2=0),
 completion_progress INTEGER,
 assignment_ticks BIGINT CHECK(assignment_ticks BETWEEN 0 AND 3155378975999999999),
 removed_ticks BIGINT CHECK(removed_ticks BETWEEN 0 AND 3155378975999999999),
 progress0 INTEGER,
 progress1 INTEGER,
 progress2 INTEGER,
 progress3 INTEGER,
 progress4 INTEGER,
 PRIMARY KEY(cid,id)
);
