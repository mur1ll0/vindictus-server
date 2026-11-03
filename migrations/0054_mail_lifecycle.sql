-- Mail lifecycle (PlayerService.ItemServiceCore MailBox/MailFactory, B5 correio). game_mail keeps
-- the dbo.Mail reference layout of 0032; dbo.MailItem (14 columns, MailItemID identity, no FK in
-- the source schema) holds the attachments, whose ItemID is a system item of game_items (owner
-- NULL, null location) or -1 for a GM mail. Strings stay raw UTF-16LE and datetimes keep SQL
-- datetime 1/300 s units; ExpireDateTime is datetime2 (ticks).
CREATE SEQUENCE game_mail_item_id;
CREATE TABLE game_mail_items(
 mail_item_id BIGINT PRIMARY KEY DEFAULT nextval('game_mail_item_id'),
 mail_id BIGINT NOT NULL DEFAULT 0,
 item_id BIGINT NOT NULL,
 item_class_ex_utf16 BYTEA NOT NULL CHECK(octet_length(item_class_ex_utf16)<=2048 AND octet_length(item_class_ex_utf16)%2=0),
 item_count INTEGER NOT NULL,
 received BOOLEAN NOT NULL DEFAULT false,
 color1 INTEGER NOT NULL,
 color2 INTEGER NOT NULL,
 color3 INTEGER NOT NULL,
 max_durability_bonus INTEGER NOT NULL,
 reduced_durability INTEGER NOT NULL DEFAULT 0,
 is_character_binded BOOLEAN NOT NULL DEFAULT false,
 expire_ticks BIGINT CHECK(expire_ticks BETWEEN 0 AND 3155378975999999999),
 received_units300 BIGINT CHECK(received_units300 BETWEEN -1391644800000 AND 76683386879999)
);
ALTER SEQUENCE game_mail_item_id OWNED BY game_mail_items.mail_item_id;
CREATE INDEX game_mail_items_mail ON game_mail_items(mail_id);
-- MailLogDataContext AddPostalJournal(MailID, FromCID, ToCID, out JournalID) and
-- AddPostalLedger(JournalID, Key, Value, ItemID, Count) of the log database.
CREATE SEQUENCE game_postal_journal_id;
CREATE TABLE game_postal_journals(
 journal_id BIGINT PRIMARY KEY DEFAULT nextval('game_postal_journal_id'),
 mail_id BIGINT NOT NULL,
 from_cid BIGINT NOT NULL,
 to_cid BIGINT NOT NULL,
 issued_units300 BIGINT NOT NULL DEFAULT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT
);
ALTER SEQUENCE game_postal_journal_id OWNED BY game_postal_journals.journal_id;
CREATE INDEX game_postal_journals_mail ON game_postal_journals(mail_id);
CREATE TABLE game_postal_ledgers(
 id BIGSERIAL PRIMARY KEY,
 journal_id BIGINT NOT NULL REFERENCES game_postal_journals(journal_id) ON DELETE CASCADE,
 key TEXT NOT NULL CHECK(length(key)<=64),
 value TEXT NOT NULL CHECK(length(value)<=2048),
 item_id BIGINT,
 count INTEGER
);
CREATE INDEX game_postal_ledgers_journal ON game_postal_ledgers(journal_id);
