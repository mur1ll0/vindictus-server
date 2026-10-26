-- The one-time play codes of 0044 are withdrawn (operator decision 2026-10-26): the game login
-- will check the password typed in the client's own login screen, if the client sends it.
-- accounts.game_name (an account entering an existing game user) stays.
DROP TABLE game_play_codes;
