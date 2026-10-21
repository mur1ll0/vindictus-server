-- StorySave (resume point after a server command with an ID) and StoryVariable (story
-- tokens) of the story service. RowID order is the Entry load order.
CREATE TABLE game_story_saves (
 row_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 cid BIGINT NOT NULL REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE,
 npc_utf16 BYTEA NOT NULL CHECK(octet_length(npc_utf16)<=140 AND octet_length(npc_utf16)%2=0),
 story_line_utf16 BYTEA NOT NULL CHECK(octet_length(story_line_utf16)<=140 AND octet_length(story_line_utf16)%2=0),
 cmd_utf16 BYTEA NOT NULL CHECK(octet_length(cmd_utf16)<=140 AND octet_length(cmd_utf16)%2=0)
);
-- StorySaveDict is keyed by (NpcID, StoryLineID).
CREATE UNIQUE INDEX game_story_saves_key ON game_story_saves(cid,npc_utf16,story_line_utf16);
CREATE TABLE game_story_variables (
 row_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 cid BIGINT NOT NULL REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE,
 story_line_utf16 BYTEA NOT NULL CHECK(octet_length(story_line_utf16)<=140 AND octet_length(story_line_utf16)%2=0),
 key_utf16 BYTEA NOT NULL CHECK(octet_length(key_utf16)<=140 AND octet_length(key_utf16)%2=0),
 value INTEGER NOT NULL
);
CREATE INDEX game_story_variables_cid ON game_story_variables(cid,row_id);
