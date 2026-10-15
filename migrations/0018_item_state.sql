-- Preserve source storage verbatim. Only the source-declared owner FKs are present.
CREATE TABLE game_items (
 id BIGINT PRIMARY KEY,
 item_class_utf16 BYTEA NOT NULL CHECK(octet_length(item_class_utf16)<=100 AND octet_length(item_class_utf16)%2=0),
 owner_id BIGINT REFERENCES game_item_collections(owner_id) ON UPDATE CASCADE ON DELETE CASCADE,
 count INTEGER NOT NULL,
 collection SMALLINT NOT NULL CHECK(collection BETWEEN 0 AND 255),
 slot INTEGER NOT NULL,
 expire_ticks BIGINT CHECK(expire_ticks BETWEEN 0 AND 3155378975999999999),
 tradable BOOLEAN NOT NULL,
 checksum INTEGER
);
CREATE INDEX game_items_owner_idx ON game_items(owner_id);
CREATE TABLE game_item_attributes (
 id BIGINT PRIMARY KEY,
 item_id BIGINT NOT NULL,
 attribute_bytes BYTEA NOT NULL CHECK(octet_length(attribute_bytes)<=50),
 value_bytes BYTEA NOT NULL CHECK(octet_length(value_bytes)<=50),
 arg INTEGER NOT NULL,
 arg2 INTEGER NOT NULL
);
CREATE INDEX game_item_attributes_item_idx ON game_item_attributes(item_id);
CREATE TABLE game_equippables (
 id BIGINT PRIMARY KEY,
 part SMALLINT CHECK(part BETWEEN 0 AND 255),
 color1 INTEGER NOT NULL,
 color2 INTEGER NOT NULL,
 color3 INTEGER NOT NULL,
 reduced_durability INTEGER NOT NULL,
 max_durability_bonus INTEGER NOT NULL
);
CREATE TABLE game_item_storage_status (
 id BIGINT PRIMARY KEY,
 owner_id BIGINT NOT NULL REFERENCES game_item_collections(owner_id) ON UPDATE CASCADE ON DELETE CASCADE,
 storage_type SMALLINT NOT NULL CHECK(storage_type BETWEEN 0 AND 255),
 storage SMALLINT NOT NULL CHECK(storage BETWEEN 0 AND 255),
 expire_units300 BIGINT CHECK(expire_units300 BETWEEN -1391644800000 AND 76683386879999),
 storage_name_utf16 BYTEA NOT NULL CHECK(octet_length(storage_name_utf16)<=100 AND octet_length(storage_name_utf16)%2=0),
 storage_tag INTEGER NOT NULL
);
CREATE INDEX game_item_storage_owner_idx ON game_item_storage_status(owner_id);
CREATE SEQUENCE game_item_attribute_id AS bigint START WITH 1 CACHE 1 NO CYCLE;
ALTER SEQUENCE game_item_attribute_id OWNED BY game_item_attributes.id;
CREATE SEQUENCE game_item_storage_id AS bigint START WITH 1 CACHE 1 NO CYCLE;
ALTER SEQUENCE game_item_storage_id OWNED BY game_item_storage_status.id;
-- No automatic default until an explicit, complete reference bootstrap is verified.
CREATE TABLE game_item_identity (
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 initialized BOOLEAN NOT NULL DEFAULT FALSE,
 attribute_last_value BIGINT CHECK(attribute_last_value BETWEEN 0 AND 9223372036854775806),
 storage_last_value BIGINT CHECK(storage_last_value BETWEEN 0 AND 9223372036854775806),
 source_item_count INTEGER CHECK(source_item_count BETWEEN 0 AND 50000),
 source_attribute_count INTEGER CHECK(source_attribute_count BETWEEN 0 AND 50000),
 source_equipment_count INTEGER CHECK(source_equipment_count BETWEEN 0 AND 50000),
 source_storage_count INTEGER CHECK(source_storage_count BETWEEN 0 AND 50000),
 CHECK(NOT initialized OR (source_item_count IS NOT NULL AND source_attribute_count IS NOT NULL AND source_equipment_count IS NOT NULL AND source_storage_count IS NOT NULL))
);
INSERT INTO game_item_identity(singleton) VALUES(TRUE);
CREATE FUNCTION item_attribute_ascii_key(value BYTEA) RETURNS TEXT
LANGUAGE plpgsql IMMUTABLE STRICT AS $$
DECLARE result TEXT := ''; pos INTEGER; code INTEGER;
BEGIN
 IF octet_length(value)>50 THEN RETURN NULL; END IF;
 IF octet_length(value)>0 THEN
  FOR pos IN 0..octet_length(value)-1 LOOP
   code:=get_byte(value,pos);
   IF code<32 OR code>126 THEN RETURN NULL; END IF;
   result:=result||chr(code);
  END LOOP;
 END IF;
 RETURN translate(rtrim(result,' '),'ABCDEFGHIJKLMNOPQRSTUVWXYZ','abcdefghijklmnopqrstuvwxyz');
END;
$$;
ALTER TABLE game_item_attributes ADD COLUMN attribute_key TEXT COLLATE "C" GENERATED ALWAYS AS(item_attribute_ascii_key(attribute_bytes)) STORED;
ALTER TABLE game_item_attributes ADD CONSTRAINT game_item_attribute_profile CHECK(attribute_key IS NOT NULL);
CREATE UNIQUE INDEX game_item_attributes_source_key ON game_item_attributes(item_id,attribute_key);
