-- Recovered HomeCafeBonus: composite primary key, CID default0, no FK or identity.
CREATE TABLE game_home_cafe_bonus (
 uid INTEGER NOT NULL,
 cid BIGINT NOT NULL DEFAULT 0,
 bonus_type SMALLINT NOT NULL CHECK(bonus_type BETWEEN 0 AND 255),
 expired_time_units300 BIGINT NOT NULL CHECK(expired_time_units300 BETWEEN -1391644800000 AND 76683386879999),
 PRIMARY KEY(uid,cid,bonus_type)
);
CREATE INDEX game_home_cafe_bonus_type ON game_home_cafe_bonus(bonus_type);
