-- heroesShare ChannelBuff: the hot spring potion of each channel and town
-- (ChannelBuffGet/ChannelBuffSet); times are .NET ticks (UTC). Rows outlive their
-- character, as in the shared source table.
CREATE TABLE game_channel_buffs (
 channel SMALLINT NOT NULL,
 town SMALLINT NOT NULL CHECK(town BETWEEN 0 AND 255),
 item_class TEXT NOT NULL CHECK(char_length(item_class) BETWEEN 1 AND 50),
 cid BIGINT NOT NULL,
 expire_ticks BIGINT CHECK(expire_ticks BETWEEN 0 AND 3155378975999999999),
 change_ticks BIGINT CHECK(change_ticks BETWEEN 0 AND 3155378975999999999),
 PRIMARY KEY(channel, town, item_class)
);
