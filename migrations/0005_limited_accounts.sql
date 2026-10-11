-- Source has one nvarchar(24) NOT NULL column and no unique key.
-- Preserve UTF16 bytes and duplicate rows; generated equality only covers printable ASCII.
CREATE FUNCTION limited_ascii_key(value BYTEA) RETURNS TEXT
LANGUAGE plpgsql IMMUTABLE STRICT AS $$
DECLARE
    text_value TEXT := '';
    pos INTEGER;
    byte_value INTEGER;
BEGIN
    IF octet_length(value) > 48 OR octet_length(value) % 2 <> 0 THEN RETURN NULL; END IF;
    IF octet_length(value) > 0 THEN
        FOR pos IN 0..octet_length(value)-1 BY 2 LOOP
            byte_value := get_byte(value, pos);
            IF get_byte(value,pos+1) <> 0 OR byte_value < 32 OR byte_value > 126 THEN RETURN NULL; END IF;
            text_value := text_value || chr(byte_value);
        END LOOP;
    END IF;
    RETURN translate(rtrim(text_value,' '),'ABCDEFGHIJKLMNOPQRSTUVWXYZ','abcdefghijklmnopqrstuvwxyz');
END;
$$;
CREATE TABLE game_limited_accounts (
    local_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    nexon_id_utf16 BYTEA NOT NULL CHECK (octet_length(nexon_id_utf16)<=48 AND octet_length(nexon_id_utf16)%2=0),
    name_key TEXT COLLATE "C" GENERATED ALWAYS AS (limited_ascii_key(nexon_id_utf16)) STORED
);
-- Intentionally non-unique: SingleOrDefault must reject multiple equivalent rows.
CREATE INDEX limited_accounts_key ON game_limited_accounts(name_key);
