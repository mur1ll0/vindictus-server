-- Laboratory messenger (B5 MS3b, decision D-S1): friend categories and friendships per
-- account (NexonSN = game_users.id). Not a recovered legacy table: the original lists lived in
-- the Nexon Messenger service. One row per direction; state 0 = requested by the owner,
-- 1 = waiting for the owner's answer, 2 = friends, 3 = blocked by the owner.
CREATE TABLE game_messenger_categories(
 nexon_sn INTEGER NOT NULL REFERENCES game_users(id) ON DELETE CASCADE,
 category_id INTEGER NOT NULL CHECK(category_id BETWEEN 1 AND 1000),
 name TEXT NOT NULL CHECK(char_length(name) BETWEEN 1 AND 32),
 PRIMARY KEY(nexon_sn, category_id)
);
CREATE TABLE game_messenger_friends(
 nexon_sn INTEGER NOT NULL REFERENCES game_users(id) ON DELETE CASCADE,
 friend_sn INTEGER NOT NULL REFERENCES game_users(id) ON DELETE CASCADE,
 state SMALLINT NOT NULL CHECK(state BETWEEN 0 AND 3),
 category_id INTEGER NOT NULL DEFAULT 0 CHECK(category_id BETWEEN 0 AND 1000),
 memo TEXT CHECK(memo IS NULL OR char_length(memo)<=256),
 created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
 PRIMARY KEY(nexon_sn, friend_sn),
 CHECK(friend_sn<>nexon_sn)
);
CREATE INDEX game_messenger_friends_friend ON game_messenger_friends(friend_sn);
