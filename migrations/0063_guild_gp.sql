-- Guild GP and levels (B5 social G4): dbo.GuildDailyGainGP (oidGuild, GPGainType, Point; the
-- MERGE of UpdateInGameGuildDailyGainGP keys it by guild and gain type) and
-- dbo.GuildCharacterMaxLevel (CID, MaxLevel: the highest guild level whose level-up reward the
-- character got; UpdateGuildCharacterMaxLevel keeps the greater value). No foreign keys, as the
-- source schema.
CREATE TABLE game_guild_daily_gain_gp(
 oid_guild INTEGER NOT NULL,
 gp_gain_type SMALLINT NOT NULL CHECK(gp_gain_type BETWEEN 0 AND 255),
 point INTEGER NOT NULL,
 PRIMARY KEY(oid_guild, gp_gain_type)
);
CREATE TABLE game_guild_character_max_level(
 cid BIGINT PRIMARY KEY,
 max_level INTEGER NOT NULL
);
