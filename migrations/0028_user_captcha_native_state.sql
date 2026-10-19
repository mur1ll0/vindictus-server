-- Operational provenance only; not additional legacy UserCaptcha fields.
-- Before upgrading the normal volume, verify the complete schema27 source snapshot.
-- No production Create/Save route existed in schema27, so initialized rows are the source set.
ALTER TABLE game_user_captcha_preservation ADD COLUMN source_uids INTEGER[] NOT NULL DEFAULT '{}';
ALTER TABLE game_user_captcha_preservation ADD CONSTRAINT game_captcha_source_uid_bound CHECK(cardinality(source_uids)<=100000);
UPDATE game_user_captcha_preservation SET source_uids=ARRAY(SELECT uid FROM game_user_captcha ORDER BY uid);
CREATE TABLE game_user_captcha_native_state (
 uid INTEGER PRIMARY KEY REFERENCES game_lab_account_origins(uid) ON DELETE CASCADE,
 last_cid BIGINT NOT NULL,
 origin_fingerprint BYTEA NOT NULL CHECK(octet_length(origin_fingerprint)=32),
 saved_sha256 BYTEA NOT NULL CHECK(octet_length(saved_sha256)=32),
 revision BIGINT NOT NULL CHECK(revision>0)
);
