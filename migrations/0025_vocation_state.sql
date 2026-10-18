-- Source dbo.Vocation and dbo.VocationSkill; SQL Server reference remains read-only.
CREATE TABLE game_vocations(
 cid BIGINT PRIMARY KEY REFERENCES game_characters(id) ON UPDATE CASCADE ON DELETE CASCADE,
 vocation_class INTEGER NOT NULL,vocation_level INTEGER NOT NULL,vocation_exp INTEGER NOT NULL,
 last_transform_ticks BIGINT CHECK(last_transform_ticks BETWEEN 0 AND 3155378975999999999)
);
-- RowID is the only unique key. Duplicate ordinal SkillID values are retained.
CREATE TABLE game_vocation_skills(
 row_id BIGINT PRIMARY KEY,cid BIGINT NOT NULL REFERENCES game_vocations(cid) ON UPDATE CASCADE ON DELETE CASCADE,
 skill_utf16 BYTEA NOT NULL CHECK(octet_length(skill_utf16)<=100 AND octet_length(skill_utf16)%2=0),
 rank INTEGER NOT NULL
);
CREATE INDEX game_vocation_skills_cid_idx ON game_vocation_skills(cid);
CREATE SEQUENCE game_vocation_skill_row_id AS bigint MINVALUE 1 MAXVALUE 9223372036854775807 START 1 CACHE 1 NO CYCLE;
ALTER SEQUENCE game_vocation_skill_row_id OWNED BY game_vocation_skills.row_id;
CREATE TABLE game_vocation_preservation(
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),initialized BOOLEAN NOT NULL DEFAULT FALSE,
 source_last_value BIGINT CHECK(source_last_value BETWEEN 0 AND 9223372036854775806),
 source_sha256 BYTEA CHECK(octet_length(source_sha256)=32),
 CHECK(NOT initialized OR source_sha256 IS NOT NULL)
);
INSERT INTO game_vocation_preservation(singleton) VALUES(TRUE);
