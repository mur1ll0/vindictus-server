-- Recovered referential actions; forward-only because 0007 has already been applied in the laboratory.
ALTER TABLE game_character_info DROP CONSTRAINT game_character_info_id_fkey;
ALTER TABLE game_character_info ADD CONSTRAINT game_character_info_id_fkey
    FOREIGN KEY (id) REFERENCES game_characters(id) ON DELETE CASCADE ON UPDATE CASCADE;
ALTER TABLE game_last_connect_info DROP CONSTRAINT game_last_connect_info_id_fkey;
ALTER TABLE game_last_connect_info ADD CONSTRAINT game_last_connect_info_id_fkey
    FOREIGN KEY (id) REFERENCES game_character_info(id) ON DELETE CASCADE ON UPDATE NO ACTION;