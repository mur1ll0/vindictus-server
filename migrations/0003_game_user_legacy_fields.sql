-- Lossless source representations. No timezone inference and no credential conversion.
ALTER TABLE game_users
    ADD COLUMN last_login_ip_utf16 BYTEA CHECK (
        octet_length(last_login_ip_utf16) <= 100 AND octet_length(last_login_ip_utf16) % 2 = 0),
    ADD COLUMN ip_lock_time_ticks BIGINT CHECK (
        ip_lock_time_ticks BETWEEN 0 AND 3155378975999999999),
    ADD COLUMN facebook_token_bytes BYTEA CHECK (octet_length(facebook_token_bytes) <= 255),
    ADD COLUMN is_dormant_user_event_units300 BIGINT CHECK (
        is_dormant_user_event_units300 BETWEEN -1391644800000 AND 76683386879999),
    ADD COLUMN second_password_utf16 BYTEA CHECK (
        octet_length(second_password_utf16) <= 512 AND octet_length(second_password_utf16) % 2 = 0),
    ADD COLUMN second_password_fail_count INTEGER,
    ADD COLUMN second_password_last_fail_ticks BIGINT CHECK (
        second_password_last_fail_ticks BETWEEN 0 AND 3155378975999999999),
    ADD COLUMN legacy_last_update BYTEA CHECK (octet_length(legacy_last_update) = 8),
    ADD COLUMN is_dormant_user_units300 BIGINT CHECK (
        is_dormant_user_units300 BETWEEN -1391644800000 AND 76683386879999),
    ADD COLUMN create_time_units300 BIGINT CHECK (
        create_time_units300 BETWEEN -1391644800000 AND 76683386879999);
-- Existing rows remain NULL: do not invent source CreateTime from persisted_at.
-- New provisioning omits the column: UTC clock rounded to source 1/300-second units.
-- Snapshot insertion supplies it explicitly, including NULL.
ALTER TABLE game_users ALTER COLUMN create_time_units300 SET DEFAULT (
    floor((extract(epoch FROM clock_timestamp()) + 2208988800) * 300 + 0.5)::BIGINT
);
