-- Guild storage (B5 G6b): heroes GuildStorageItems (an item row without an owner, linked to
-- its guild), GuildStorageStatus (one row per guild with TotalStorageCount, the slots of the
-- guild's single bag) and GuildStorageSetting (InitGuildStorageSetting: AccessLimitTag
-- 898779, GoldLimit 0). No foreign keys, as the source schema.
CREATE TABLE game_guild_storage_items(
 pk BIGSERIAL PRIMARY KEY,
 id BIGINT NOT NULL UNIQUE,
 guild_id INTEGER NOT NULL
);
CREATE INDEX game_guild_storage_items_guild ON game_guild_storage_items(guild_id);
CREATE TABLE game_guild_storage_status(
 id BIGSERIAL PRIMARY KEY,
 guild_id INTEGER NOT NULL UNIQUE,
 total_storage_count INTEGER NOT NULL
);
CREATE TABLE game_guild_storage_setting(
 guild_id INTEGER PRIMARY KEY,
 access_limit_tag BIGINT NOT NULL,
 gold_limit INTEGER NOT NULL
);
