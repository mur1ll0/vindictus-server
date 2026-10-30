-- heroes Pet and PetSkill (PlayerService PetManager): pets belong to the account
-- (OwnerUID = game_users.id), not to a character. Times are .NET ticks (UTC).
-- PetAccessory is not created: EquipPetAccessory has no caller in 1.69.
CREATE TABLE game_pets (
 id BIGINT PRIMARY KEY,
 owner_uid INTEGER NOT NULL,
 name TEXT NOT NULL CHECK(char_length(name) BETWEEN 1 AND 32),
 pet_type INTEGER NOT NULL,
 stat_type INTEGER NOT NULL,
 slot INTEGER NOT NULL CHECK(slot BETWEEN 0 AND 99),
 status SMALLINT NOT NULL CHECK(status BETWEEN 0 AND 255),
 level INTEGER NOT NULL,
 exp INTEGER NOT NULL,
 total_exp INTEGER NOT NULL,
 hunger INTEGER NOT NULL,
 remain_active_time INTEGER,
 deactive_start_ticks BIGINT CHECK(deactive_start_ticks BETWEEN 0 AND 3155378975999999999),
 expired_ticks BIGINT CHECK(expired_ticks BETWEEN 0 AND 3155378975999999999),
 -- SwapSlot exchanges two slots in one statement.
 CONSTRAINT game_pets_owner_slot UNIQUE(owner_uid, slot) DEFERRABLE INITIALLY IMMEDIATE
);
-- CheckPetName: names are unique per account without case (ToLower).
CREATE UNIQUE INDEX game_pets_owner_name ON game_pets(owner_uid, lower(name));
-- Cash skills (slots 101 and 102); default skills come from DefaultPetSkillInfo.
CREATE TABLE game_pet_skills (
 row_id BIGINT PRIMARY KEY,
 pet_id BIGINT NOT NULL REFERENCES game_pets(id) ON DELETE CASCADE,
 slot INTEGER NOT NULL,
 skill_id INTEGER NOT NULL,
 open_level INTEGER NOT NULL,
 cool_time_type SMALLINT NOT NULL CHECK(cool_time_type BETWEEN 0 AND 255),
 cool_time INTEGER NOT NULL DEFAULT 0,
 last_use_ticks BIGINT NOT NULL,
 expired_ticks BIGINT CHECK(expired_ticks BETWEEN 0 AND 3155378975999999999),
 UNIQUE(pet_id, slot, skill_id)
);
-- UserSharedItem Type 2 (Pet_food_V2): the food each account shares with its pets,
-- Argument = PetFeedInfo.FeedType.
CREATE TABLE game_pet_feeds (
 owner_uid INTEGER NOT NULL,
 feed_type SMALLINT NOT NULL CHECK(feed_type BETWEEN 0 AND 255),
 count INTEGER NOT NULL CHECK(count > 0),
 PRIMARY KEY(owner_uid, feed_type)
);
-- heroesLog PetLedger (AddPetLedger): CharacterUID is the account UID.
CREATE TABLE game_pet_ledger (
 id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 pet_id BIGINT NOT NULL,
 owner_uid INTEGER NOT NULL,
 operation TEXT NOT NULL CHECK(char_length(operation) BETWEEN 1 AND 50),
 event TEXT NOT NULL CHECK(char_length(event) BETWEEN 1 AND 50),
 arg1 TEXT CHECK(char_length(arg1) <= 100),
 arg2 TEXT CHECK(char_length(arg2) <= 100),
 logged_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX game_pet_ledger_owner ON game_pet_ledger(owner_uid, id);
