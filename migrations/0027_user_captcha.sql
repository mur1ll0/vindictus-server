-- dbo.UserCaptcha: six fields, UID PK, no identity or foreign key in source.
-- Preserve datetime as signed 1/300-second units since 1900, without timezone inference.
CREATE TABLE game_user_captcha (
 uid INTEGER PRIMARY KEY,
 punished_count SMALLINT CHECK(punished_count BETWEEN 0 AND 255),
 failure_count SMALLINT CHECK(failure_count BETWEEN 0 AND 255),
 last_failure_units300 BIGINT CHECK(last_failure_units300 BETWEEN -1391644800000 AND 76683386879999),
 last_succeed_units300 BIGINT CHECK(last_succeed_units300 BETWEEN -1391644800000 AND 76683386879999),
 condition SMALLINT CHECK(condition BETWEEN 0 AND 255)
);
CREATE TABLE game_user_captcha_preservation (
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 source_sha256 BYTEA NOT NULL CHECK(octet_length(source_sha256)=32)
);
