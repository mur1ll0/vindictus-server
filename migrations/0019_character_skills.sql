-- Skill has no identity. SPSkill owns its independent SQL Server identity.
CREATE TABLE game_skills (
 row_id BIGINT PRIMARY KEY,
 cid BIGINT NOT NULL REFERENCES game_characters(id) ON UPDATE CASCADE ON DELETE CASCADE,
 skill_utf16 BYTEA NOT NULL CHECK(octet_length(skill_utf16)<=100 AND octet_length(skill_utf16)%2=0),
 rank INTEGER NOT NULL,
 locked SMALLINT NOT NULL CHECK(locked BETWEEN 0 AND 255),
 current_ap INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX game_skills_cid_idx ON game_skills(cid);
CREATE TABLE game_sp_skills (
 row_id INTEGER PRIMARY KEY,
 cid BIGINT NOT NULL DEFAULT -1 REFERENCES game_characters(id) ON UPDATE CASCADE ON DELETE CASCADE,
 slot_id INTEGER NOT NULL DEFAULT 0,
 skill_utf16 BYTEA DEFAULT decode('','hex') CHECK(octet_length(skill_utf16)<=100 AND octet_length(skill_utf16)%2=0)
);
CREATE INDEX game_sp_skills_cid_idx ON game_sp_skills(cid);
CREATE SEQUENCE game_sp_skill_row_id AS integer INCREMENT BY 1 MINVALUE 1 MAXVALUE 2147483647 START WITH 1 CACHE 1 NO CYCLE;
ALTER SEQUENCE game_sp_skill_row_id OWNED BY game_sp_skills.row_id;
CREATE TABLE game_skill_identity (
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 initialized BOOLEAN NOT NULL DEFAULT FALSE,
 source_last_value INTEGER CHECK(source_last_value BETWEEN 0 AND 2147483646)
);
INSERT INTO game_skill_identity(singleton) VALUES(TRUE);
