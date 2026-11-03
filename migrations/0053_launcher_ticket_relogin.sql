-- Launcher ticket re-login (trilha L, LA4 of 2026-11-03): the client keeps the ticket of its
-- command line for the whole game process, and "Go Back" on the character selection logs out on
-- the same connection; the next Start Game sends the same ticket again. A spent ticket was refused
-- (GAMEUI_LOGIN_FAIL_PERMISSION_FAIL). A ticket now stays good for re-logins of the same game while
-- its launcher session lasts, only from the address of its first login; the next Play replaces it.
ALTER TABLE game_launcher_tickets ADD COLUMN redeemed_from INET;
