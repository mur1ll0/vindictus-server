-- Allocation is deliberately disabled until an explicit source identity profile is installed.
CREATE SEQUENCE game_queued_item_row_id AS integer INCREMENT BY 1 MINVALUE 1 MAXVALUE 2147483647 START WITH 1 CACHE 1 NO CYCLE;
ALTER SEQUENCE game_queued_item_row_id OWNED BY game_queued_items.row_id;
CREATE TABLE game_queue_identity (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    initialized BOOLEAN NOT NULL DEFAULT FALSE,
    source_last_value INTEGER CHECK (source_last_value BETWEEN 0 AND 2147483646)
);
INSERT INTO game_queue_identity(singleton) VALUES(TRUE);
-- These are the recovered defaults; LogDate is supplied as an explicit UTC clock sample.
ALTER TABLE game_queued_items
    ALTER COLUMN is_character_binded SET DEFAULT FALSE,
    ALTER COLUMN color1 SET DEFAULT -1,
    ALTER COLUMN color2 SET DEFAULT -1,
    ALTER COLUMN color3 SET DEFAULT -1,
    ALTER COLUMN reduced_durability SET DEFAULT 0,
    ALTER COLUMN max_durability_bonus SET DEFAULT 0,
    ALTER COLUMN processed SET DEFAULT FALSE;
CREATE TABLE game_dormant_item_runs (
    operation_id UUID PRIMARY KEY,
    request BYTEA NOT NULL CHECK (octet_length(request) <= 5000),
    row_id INTEGER
);
-- No FK: a processed/removed queue row must not erase the replay receipt.
