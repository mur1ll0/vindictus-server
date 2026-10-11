-- All source columns and the two recovered foreign keys. No new-user/queue allocator.

CREATE TABLE game_characters (
    id BIGINT NOT NULL PRIMARY KEY
);

CREATE TABLE game_character_info (
    id BIGINT NOT NULL PRIMARY KEY REFERENCES game_characters(id),
    uid INTEGER NOT NULL,
    character_sn INTEGER NOT NULL,
    name_utf16 BYTEA CHECK (octet_length(name_utf16)<=100 AND octet_length(name_utf16)%2=0),
    class SMALLINT NOT NULL CHECK (class BETWEEN 0 AND 255),
    level INTEGER NOT NULL,
    exp BIGINT NOT NULL,
    str INTEGER NOT NULL,
    dex INTEGER NOT NULL,
    int INTEGER NOT NULL,
    will INTEGER NOT NULL,
    luck INTEGER NOT NULL,
    hp INTEGER NOT NULL,
    stamina INTEGER NOT NULL,
    dirty BOOLEAN NOT NULL,
    play_time BIGINT NOT NULL,
    selected_title INTEGER,
    ap INTEGER NOT NULL,
    total_used_ap INTEGER NOT NULL,
    last_ap_update_units300 BIGINT NOT NULL CHECK (last_ap_update_units300 BETWEEN -1391644800000 AND 76683386879999),
    permission INTEGER NOT NULL,
    status INTEGER NOT NULL,
    quote_utf16 BYTEA CHECK (octet_length(quote_utf16)<=100 AND octet_length(quote_utf16)%2=0),
    delete_time_units300 BIGINT CHECK (delete_time_units300 BETWEEN -1391644800000 AND 76683386879999),
    region_code INTEGER NOT NULL,
    is_connected BOOLEAN NOT NULL,
    legacy_last_update BYTEA CHECK (octet_length(legacy_last_update)=8),
    create_time_ticks BIGINT NOT NULL CHECK (create_time_ticks BETWEEN 0 AND 3155378975999999999),
    free_match_win_count INTEGER NOT NULL,
    free_match_lose_count INTEGER NOT NULL,
    use_preset BOOLEAN NOT NULL,
    is_premium BOOLEAN NOT NULL,
    free_title_name_utf16 BYTEA CHECK (octet_length(free_title_name_utf16)<=100 AND octet_length(free_title_name_utf16)%2=0),
    today_play_time INTEGER,
    channel SMALLINT,
    order_num SMALLINT CHECK (order_num BETWEEN 0 AND 255),
    map_state_info SMALLINT NOT NULL CHECK (map_state_info BETWEEN 0 AND 255)
);

CREATE TABLE game_last_connect_info (
    id BIGINT NOT NULL PRIMARY KEY REFERENCES game_character_info(id),
    last_connect_date_units300 BIGINT NOT NULL CHECK (last_connect_date_units300 BETWEEN -1391644800000 AND 76683386879999),
    last_cafe_bonus_units300 BIGINT CHECK (last_cafe_bonus_units300 BETWEEN -1391644800000 AND 76683386879999),
    last_cafe_login_units300 BIGINT CHECK (last_cafe_login_units300 BETWEEN -1391644800000 AND 76683386879999),
    last_cafe_logout_units300 BIGINT CHECK (last_cafe_logout_units300 BETWEEN -1391644800000 AND 76683386879999),
    last_home_cafe_login_units300 BIGINT CHECK (last_home_cafe_login_units300 BETWEEN -1391644800000 AND 76683386879999),
    last_home_cafe_logout_units300 BIGINT CHECK (last_home_cafe_logout_units300 BETWEEN -1391644800000 AND 76683386879999),
    last_logout_units300 BIGINT CHECK (last_logout_units300 BETWEEN -1391644800000 AND 76683386879999)
);

CREATE TABLE game_queued_items (
    row_id INTEGER NOT NULL PRIMARY KEY,
    cid BIGINT NOT NULL,
    item_class_ex_utf16 BYTEA NOT NULL CHECK (octet_length(item_class_ex_utf16)<=2048 AND octet_length(item_class_ex_utf16)%2=0),
    count INTEGER NOT NULL,
    is_character_binded BOOLEAN NOT NULL,
    mail_title_utf16 BYTEA CHECK (octet_length(mail_title_utf16)<=512 AND octet_length(mail_title_utf16)%2=0),
    mail_content_utf16 BYTEA CHECK (octet_length(mail_content_utf16)<=2048 AND octet_length(mail_content_utf16)%2=0),
    color1 INTEGER NOT NULL,
    color2 INTEGER NOT NULL,
    color3 INTEGER NOT NULL,
    reduced_durability INTEGER NOT NULL,
    max_durability_bonus INTEGER NOT NULL,
    item_id BIGINT,
    processed BOOLEAN NOT NULL,
    event_code_utf16 BYTEA CHECK (octet_length(event_code_utf16)<=100 AND octet_length(event_code_utf16)%2=0),
    processed_time_units300 BIGINT CHECK (processed_time_units300 BETWEEN -1391644800000 AND 76683386879999),
    expire_time_units300 BIGINT CHECK (expire_time_units300 BETWEEN -1391644800000 AND 76683386879999),
    log_date_units300 BIGINT CHECK (log_date_units300 BETWEEN -1391644800000 AND 76683386879999)
);

CREATE INDEX game_character_info_uid_order ON game_character_info(uid,COALESCE(order_num,255),id);

CREATE INDEX game_queued_items_cid ON game_queued_items(cid);
