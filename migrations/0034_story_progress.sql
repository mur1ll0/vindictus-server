-- StoryProgress (RowID is the entity ID assigned by OpenStoryLine, not an identity) and
-- StoryGuide of the story service. LastUpdate keeps SQL datetime 1/300 s units.
CREATE TABLE game_story_progress (
 row_id BIGINT PRIMARY KEY,
 cid BIGINT NOT NULL REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE,
 story_line_utf16 BYTEA NOT NULL CHECK(octet_length(story_line_utf16)<=140 AND octet_length(story_line_utf16)%2=0),
 phase_utf16 BYTEA CHECK(octet_length(phase_utf16)<=100 AND octet_length(phase_utf16)%2=0),
 status SMALLINT NOT NULL CHECK(status BETWEEN 0 AND 255),
 last_update_units300 BIGINT NOT NULL CHECK(last_update_units300 BETWEEN -1391644800000 AND 76683386879999)
);
-- StoryEntity.ProgressDict.Add throws on a duplicate StoryLineID of one character.
CREATE UNIQUE INDEX game_story_progress_line ON game_story_progress(cid,story_line_utf16);
CREATE TABLE game_story_guides (
 cid BIGINT PRIMARY KEY REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE,
 guide_target_utf16 BYTEA CHECK(octet_length(guide_target_utf16)<=100 AND octet_length(guide_target_utf16)%2=0),
 guide_message_utf16 BYTEA CHECK(octet_length(guide_message_utf16)<=400 AND octet_length(guide_message_utf16)%2=0)
);
