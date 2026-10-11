-- Full source columns for these two tables. Neither has foreign keys in the reference.
CREATE TABLE game_friend_loyalty (
    recommender_uid INTEGER PRIMARY KEY,
    recommender_cid BIGINT,
    friend_cid BIGINT,
    recommend_flag INTEGER NOT NULL,
    update_time_units300 BIGINT NOT NULL CHECK (update_time_units300 BETWEEN -1391644800000 AND 76683386879999)
);
-- Explicit source IDs only: achievement creation/identity allocation is not implemented here.
CREATE TABLE game_user_achievements (
    id INTEGER PRIMARY KEY,
    uid INTEGER NOT NULL,
    name_utf16 BYTEA NOT NULL CHECK (octet_length(name_utf16)<=48 AND octet_length(name_utf16)%2=0),
    achievement_utf16 BYTEA NOT NULL CHECK (octet_length(achievement_utf16)<=100 AND octet_length(achievement_utf16)%2=0),
    name_key TEXT COLLATE "C" GENERATED ALWAYS AS (limited_ascii_key(name_utf16)) STORED
);
CREATE INDEX user_achievements_pending_name ON game_user_achievements(name_key) WHERE uid=-1;
-- Operational receipts are separate from source data. The same UUID must carry the same request.
CREATE TABLE game_login_account_effect_runs (
    operation_id UUID PRIMARY KEY,
    uid INTEGER NOT NULL,
    name_key TEXT COLLATE "C" NOT NULL,
    request TEXT COLLATE "C" NOT NULL CHECK (octet_length(request)<=4096),
    revision_after BIGINT NOT NULL CHECK (revision_after>=0),
    is_dormant BOOLEAN NOT NULL,
    is_return BOOLEAN NOT NULL
);
CREATE INDEX login_account_effect_runs_uid ON game_login_account_effect_runs(uid);

