-- Complete AttendanceNewbieInfo reference;UID PK,no identity/FK,datetime2(7).
CREATE TABLE game_newbie_attendance_info(
 uid INTEGER PRIMARY KEY,event_version INTEGER NOT NULL,
 start_ticks BIGINT NOT NULL CHECK(start_ticks BETWEEN 0 AND 3155378975999999999),
 end_ticks BIGINT NOT NULL CHECK(end_ticks BETWEEN 0 AND 3155378975999999999)
);
CREATE TABLE game_newbie_attendance_preservation(
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 source_sha256 BYTEA NOT NULL CHECK(octet_length(source_sha256)=32),
 source_uids INTEGER[] NOT NULL CHECK(cardinality(source_uids)<=100000)
);
