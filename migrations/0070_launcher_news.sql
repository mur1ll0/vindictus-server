-- Launcher news (L5, docs/future/game-launcher.md 4.7): written in the panel's "Versões do cliente"
-- page, signed into the update store's news.json (newest 20 visible items) for the launcher. A hidden
-- item leaves the feed and stays here; every change is in admin_audit_events.
CREATE TABLE launcher_news(
 id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 title TEXT NOT NULL CHECK(char_length(title) BETWEEN 1 AND 120),
 body TEXT NOT NULL CHECK(char_length(body) BETWEEN 1 AND 2000),
 published_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
 account_id UUID REFERENCES accounts(id),
 hidden BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX launcher_news_visible ON launcher_news(published_at DESC, id DESC) WHERE NOT hidden;
