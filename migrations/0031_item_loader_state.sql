-- Full source row shapes;fresh-loader absence is a separate guarded operation.
CREATE SEQUENCE game_quick_slot_id AS BIGINT START 1;
CREATE TABLE game_quick_slots(
 row_id BIGINT NOT NULL PRIMARY KEY DEFAULT nextval('game_quick_slot_id'),
 owner_id BIGINT REFERENCES game_characters(id) ON UPDATE CASCADE ON DELETE CASCADE NOT NULL,
 item_class_utf16 BYTEA CHECK(octet_length(item_class_utf16)<=100 AND octet_length(item_class_utf16)%2=0) NOT NULL,
 slot INTEGER NOT NULL
);
ALTER SEQUENCE game_quick_slot_id OWNED BY game_quick_slots.row_id;
CREATE SEQUENCE game_expanded_slot_id AS INTEGER START 1;
CREATE TABLE game_expanded_slots(
 row_id INTEGER NOT NULL PRIMARY KEY DEFAULT nextval('game_expanded_slot_id'),
 cid BIGINT NOT NULL,
 slot INTEGER NOT NULL
);
ALTER SEQUENCE game_expanded_slot_id OWNED BY game_expanded_slots.row_id;
CREATE SEQUENCE game_manufacture_id AS INTEGER START 1;
CREATE TABLE game_manufacture(
 row_id INTEGER NOT NULL PRIMARY KEY DEFAULT nextval('game_manufacture_id'),
 cid BIGINT NOT NULL,
 manufacture_utf16 BYTEA CHECK(octet_length(manufacture_utf16)<=32 AND octet_length(manufacture_utf16)%2=0) NOT NULL,
 grade INTEGER NOT NULL,
 experience_point INTEGER NOT NULL
);
ALTER SEQUENCE game_manufacture_id OWNED BY game_manufacture.row_id;
CREATE SEQUENCE game_manufacture_recipe_id AS INTEGER START 1;
CREATE TABLE game_manufacture_recipes(
 row_id INTEGER NOT NULL PRIMARY KEY DEFAULT nextval('game_manufacture_recipe_id'),
 cid BIGINT NOT NULL,
 recipe_utf16 BYTEA CHECK(octet_length(recipe_utf16)<=256 AND octet_length(recipe_utf16)%2=0) NOT NULL,
 acquired_ticks BIGINT CHECK(acquired_ticks BETWEEN 0 AND 3155378975999999999) NOT NULL
);
ALTER SEQUENCE game_manufacture_recipe_id OWNED BY game_manufacture_recipes.row_id;
CREATE SEQUENCE game_new_recipe_id AS BIGINT START 1;
CREATE TABLE game_new_recipes(
 row_id BIGINT NOT NULL PRIMARY KEY DEFAULT nextval('game_new_recipe_id'),
 cid BIGINT REFERENCES game_item_collections(owner_id) ON UPDATE CASCADE ON DELETE CASCADE NOT NULL,
 recipe_utf16 BYTEA CHECK(octet_length(recipe_utf16)<=200 AND octet_length(recipe_utf16)%2=0) NOT NULL,
 new_until_ticks BIGINT CHECK(new_until_ticks BETWEEN 0 AND 3155378975999999999) NOT NULL
);
ALTER SEQUENCE game_new_recipe_id OWNED BY game_new_recipes.row_id;
CREATE TABLE game_item_loader_preservation(singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK(singleton),source_sha256 BYTEA NOT NULL CHECK(octet_length(source_sha256)=32),source_last BIGINT[] NOT NULL CHECK(cardinality(source_last)=5));
