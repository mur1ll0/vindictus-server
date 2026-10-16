-- Scoped creation log writers; historical log preservation remains a separate migration dependency.
-- No PK/FK/identity/uniqueness exists on these source event tables.
-- legacy_last_update holds original rowversions only; new PostgreSQL rows do not fabricate them.
CREATE FUNCTION character_event_log_ticks(units BIGINT) RETURNS BIGINT
LANGUAGE SQL IMMUTABLE STRICT AS $$
 SELECT CASE WHEN units BETWEEN -1391644800000 AND 76683386879999 THEN
  599266080000000000 + floor(units::numeric/25920000)::BIGINT*864000000000
  + (((((units%25920000)+25920000)%25920000)*10+1)/3)*10000 END
$$;
CREATE TABLE game_character_skill_log01 (
 journal_id BIGINT NOT NULL,
 event INTEGER NOT NULL,
 skill_utf16 BYTEA NOT NULL CHECK(octet_length(skill_utf16)<=100 AND octet_length(skill_utf16)%2=0),
 skill_level INTEGER NOT NULL,
 ap INTEGER NOT NULL,
 time_ticks BIGINT NOT NULL DEFAULT character_event_log_ticks(floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT) CHECK(time_ticks BETWEEN 0 AND 3155378975999999999),
 legacy_last_update BYTEA CHECK(octet_length(legacy_last_update)=8)
);
CREATE TABLE game_character_skill_log02 (LIKE game_character_skill_log01 INCLUDING DEFAULTS INCLUDING CONSTRAINTS);
CREATE TABLE game_character_title_log01 (
 journal_id BIGINT NOT NULL,
 event INTEGER NOT NULL,
 title_id INTEGER NOT NULL,
 goal_id INTEGER NOT NULL,
 value INTEGER NOT NULL,
 time_ticks BIGINT NOT NULL DEFAULT character_event_log_ticks(floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT) CHECK(time_ticks BETWEEN 0 AND 3155378975999999999),
 legacy_last_update BYTEA CHECK(octet_length(legacy_last_update)=8)
);
CREATE TABLE game_character_title_log02 (LIKE game_character_title_log01 INCLUDING DEFAULTS INCLUDING CONSTRAINTS);
