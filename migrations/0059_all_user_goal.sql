-- Collective goals (B5 eventos): AllUserGoalEvent of heroesShare (study_heroesShare.schema.json,
-- the AddAllUserEventCount procedure: one count per goal slot with the end of its window,
-- here Unix seconds of the shifted EndTime) and EventAttend of heroes (study_heroes.schema.json,
-- EventAttendUpdate MERGE on EventNo and UID; LastUpdateTime as Unix seconds). Keys follow the
-- procedures' lookups; no foreign keys, as the source schema.
CREATE TABLE game_all_user_goal_event(
 slot_id INTEGER NOT NULL PRIMARY KEY,
 goal_count INTEGER NOT NULL,
 end_seconds BIGINT NOT NULL
);
CREATE TABLE game_event_attend(
 event_no INTEGER NOT NULL,
 uid INTEGER NOT NULL,
 cid BIGINT NOT NULL,
 last_update_seconds BIGINT NOT NULL,
 PRIMARY KEY(event_no, uid)
);
