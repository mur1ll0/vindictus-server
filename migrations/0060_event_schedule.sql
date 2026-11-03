-- Event schedule of the panel (laboratory extension of the AdminClient Event table of heroes,
-- AdminClientServiceCore Event: Name, Feature, StartScript, EndScript, StartTime, EndTime,
-- PeriodBegin, PeriodEnd, StartMessage, NotifyMessage, EndMessage, NotifyInterval, UserName).
-- StartTime/EndTime are kept in UTC; the daily period is seconds of the server's local day.
CREATE TABLE game_event_schedules(
 name TEXT PRIMARY KEY CHECK (char_length(name) BETWEEN 1 AND 100),
 template TEXT CHECK (char_length(template) <= 100),
 feature TEXT CHECK (char_length(feature) <= 2000),
 start_script TEXT CHECK (char_length(start_script) <= 4000),
 end_script TEXT CHECK (char_length(end_script) <= 4000),
 start_at TIMESTAMPTZ,
 end_at TIMESTAMPTZ,
 period_begin_sec INTEGER CHECK (period_begin_sec BETWEEN 0 AND 86399),
 period_end_sec INTEGER CHECK (period_end_sec BETWEEN 0 AND 86400),
 start_message TEXT CHECK (char_length(start_message) <= 500),
 notify_message TEXT CHECK (char_length(notify_message) <= 500),
 end_message TEXT CHECK (char_length(end_message) <= 500),
 notify_interval INTEGER CHECK (notify_interval >= 0),
 created_by UUID NOT NULL REFERENCES accounts(id),
 created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
