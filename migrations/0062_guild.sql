-- In-game guilds (B5 social G1): the HeroesGuildAPI tables of heroes (study_heroes.schema.json).
-- dbo.GuildInfo (GuildSN from func_GenerateOidUserEx, GuildID nvarchar(24), GuildName
-- nvarchar(50), Intro, OwnerID, RealUserCount, dateCreate, dateClosed), dbo.GuildUser (CID,
-- GuildSN, Intro, codeGroupUserType 5/15/125/155), dbo.Guild (oidGuild, MaxMemberLimit,
-- NewbieRecommend, Level, Exp, Notice, LastDailyGPReset, TotalGP) and dbo.GuildCharacterInfo
-- (CID, Point). Names are ASCII by the naming rule and IDs are compared case-insensitively as the
-- source collation does, so they are TEXT with lower() keys; times are Unix seconds. A closed
-- guild keeps its row (dateClosed) and its ID; only an open guild holds its name. GuildUser has
-- one row per character (GuildCreate/GuildUserJoin refuse a second). No foreign keys, as the
-- source schema.
CREATE SEQUENCE game_guild_sn START 1;
CREATE TABLE game_guild_info(
 guild_sn INTEGER PRIMARY KEY DEFAULT nextval('game_guild_sn'),
 guild_id TEXT NOT NULL CHECK(char_length(guild_id) BETWEEN 1 AND 24),
 guild_name TEXT NOT NULL CHECK(char_length(guild_name) BETWEEN 1 AND 50),
 intro TEXT CHECK(char_length(intro)<=200),
 owner_cid BIGINT,
 real_user_count INTEGER NOT NULL,
 created_seconds BIGINT NOT NULL,
 modified_seconds BIGINT NOT NULL,
 closed_seconds BIGINT
);
ALTER SEQUENCE game_guild_sn OWNED BY game_guild_info.guild_sn;
CREATE UNIQUE INDEX game_guild_info_id ON game_guild_info(lower(guild_id));
CREATE UNIQUE INDEX game_guild_info_open_name ON game_guild_info(lower(guild_name)) WHERE closed_seconds IS NULL;
CREATE TABLE game_guild_user(
 cid BIGINT PRIMARY KEY,
 guild_sn INTEGER NOT NULL,
 intro TEXT CHECK(char_length(intro)<=200),
 created_seconds BIGINT NOT NULL,
 modified_seconds BIGINT NOT NULL,
 group_user_type SMALLINT NOT NULL
);
CREATE INDEX game_guild_user_guild ON game_guild_user(guild_sn);
CREATE TABLE game_guild(
 oid_guild INTEGER PRIMARY KEY,
 max_member_limit INTEGER NOT NULL DEFAULT 0,
 newbie_recommend BOOLEAN NOT NULL DEFAULT false,
 level INTEGER NOT NULL DEFAULT 1,
 exp BIGINT NOT NULL DEFAULT 0,
 notice TEXT NOT NULL DEFAULT '' CHECK(char_length(notice)<=256),
 last_daily_gp_reset_seconds BIGINT NOT NULL,
 total_gp BIGINT NOT NULL DEFAULT 0
);
CREATE TABLE game_guild_character_info(
 cid BIGINT PRIMARY KEY,
 point BIGINT NOT NULL DEFAULT 0
);
