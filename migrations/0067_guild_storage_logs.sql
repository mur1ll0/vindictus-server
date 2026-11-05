-- Guild storage logs (B5 G6f): heroes GuildStorageBriefLog (one row per guild, character,
-- operation type and day: AddGuildStorageBriefLog adds the counts) and GuildStorageItemLog
-- (AddGuildStorageItemLog keeps ten rows per character and day). Datestamp is yyyyMMdd and
-- Timestamp the seconds of the day, both UTC. No foreign keys, as the source schema.
CREATE TABLE game_guild_storage_brief_log(
 id BIGSERIAL PRIMARY KEY,
 guild_id INTEGER NOT NULL,
 character_name TEXT NOT NULL CHECK(char_length(character_name)<=32),
 operation_type SMALLINT NOT NULL CHECK(operation_type BETWEEN 0 AND 255),
 add_count INTEGER NOT NULL,
 pick_count INTEGER NOT NULL,
 date_stamp INTEGER NOT NULL,
 time_stamp INTEGER NOT NULL
);
CREATE INDEX game_guild_storage_brief_log_day ON game_guild_storage_brief_log(guild_id,date_stamp);
CREATE TABLE game_guild_storage_item_log(
 id BIGSERIAL PRIMARY KEY,
 guild_id INTEGER NOT NULL,
 character_name TEXT NOT NULL CHECK(char_length(character_name)<=32),
 operation_type SMALLINT NOT NULL CHECK(operation_type BETWEEN 0 AND 255),
 item_class TEXT NOT NULL CHECK(char_length(item_class)<=128),
 count INTEGER NOT NULL,
 date_stamp INTEGER NOT NULL,
 time_stamp INTEGER NOT NULL,
 color1 INTEGER NOT NULL DEFAULT -1,
 color2 INTEGER NOT NULL DEFAULT -1,
 color3 INTEGER NOT NULL DEFAULT -1
);
CREATE INDEX game_guild_storage_item_log_day ON game_guild_storage_item_log(guild_id,date_stamp);
