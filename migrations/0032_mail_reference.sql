-- Complete Mail reference:14fields,no identity/FK;raw UTF16 and SQL datetime units.
CREATE TABLE game_mail(
 mail_id BIGINT PRIMARY KEY,
 from_cid BIGINT NOT NULL DEFAULT -1,
 from_name_utf16 BYTEA NOT NULL CHECK(octet_length(from_name_utf16)<=512 AND octet_length(from_name_utf16)%2=0),
 to_cid BIGINT NOT NULL DEFAULT -1,
 to_name_utf16 BYTEA NOT NULL CHECK(octet_length(to_name_utf16)<=40 AND octet_length(to_name_utf16)%2=0),
 mail_type SMALLINT NOT NULL CHECK(mail_type BETWEEN 0 AND 255),
 posted_units300 BIGINT NOT NULL CHECK(posted_units300 BETWEEN -1391644800000 AND 76683386879999),
 title_utf16 BYTEA NOT NULL CHECK(octet_length(title_utf16)<=512 AND octet_length(title_utf16)%2=0),
 content_utf16 BYTEA NOT NULL CHECK(octet_length(content_utf16)<=2048 AND octet_length(content_utf16)%2=0),
 arrive_units300 BIGINT NOT NULL CHECK(arrive_units300 BETWEEN -1391644800000 AND 76683386879999),
 expire_units300 BIGINT CHECK(expire_units300 BETWEEN -1391644800000 AND 76683386879999),
 is_read BOOLEAN DEFAULT false,
 charged_gold INTEGER NOT NULL,
 is_deleted BOOLEAN NOT NULL DEFAULT false
);
CREATE INDEX game_mail_to_cid ON game_mail(to_cid);
CREATE INDEX game_mail_from_cid ON game_mail(from_cid);
CREATE TABLE game_mail_preservation(
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 source_sha256 BYTEA NOT NULL CHECK(octet_length(source_sha256)=32),
 procedure_sha256 TEXT NOT NULL CHECK(length(procedure_sha256)=64),
 source_return_cids BIGINT[] NOT NULL CHECK(cardinality(source_return_cids)<=200000)
);
