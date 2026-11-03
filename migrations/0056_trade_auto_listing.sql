-- Laboratory extension (not a recovered table): the panel's settings of the server's own
-- marketplace listings (docs/marketplace.md, "Listagens automáticas"). One row; off by
-- default. The listings themselves are game_trade_items rows of the seller CID -1.
CREATE TABLE game_trade_auto_settings(
 singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK(singleton),
 enabled BOOLEAN NOT NULL DEFAULT false,
 interval_hours INTEGER NOT NULL DEFAULT 1 CHECK(interval_hours BETWEEN 1 AND 168),
 items INTEGER NOT NULL DEFAULT 100 CHECK(items BETWEEN 1 AND 1000),
 days INTEGER NOT NULL DEFAULT 1 CHECK(days BETWEEN 1 AND 30),
 last_run_ticks BIGINT,
 last_listed INTEGER NOT NULL DEFAULT 0
);
INSERT INTO game_trade_auto_settings DEFAULT VALUES;
